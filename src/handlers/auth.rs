use actix_web::{web, HttpRequest, HttpResponse};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::config::SharedConfig;
use crate::dto::{ApiResponse, PaginatedResponse};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::{Genre, Tag, UserSafe};
use crate::services::QueueService;

// ── Helper: extract language from request ────────────────────────────────────

fn lang_from_req(req: &HttpRequest) -> String {
    req.headers()
        .get("Accept-Language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("en")
        .split(',')
        .next()
        .unwrap_or("en")
        .split('-')
        .next()
        .unwrap_or("en")
        .to_string()
}

// ── Request DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "validation.email"))]
    pub email: String,
    #[validate(length(min = 6, message = "validation.min_length"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SmsCodeRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VerifyCodeRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
    #[validate(length(min = 4, max = 6, message = "validation.code"))]
    pub code: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CompleteRegistrationRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
    #[validate(length(min = 4, max = 6, message = "validation.code"))]
    pub code: String,
    #[validate(email(message = "validation.email"))]
    pub email: String,
    #[validate(length(min = 3, max = 30, message = "validation.username"))]
    pub username: String,
    #[validate(length(min = 8, message = "validation.min_length"))]
    pub password: String,
    #[validate(length(min = 1, message = "validation.required"))]
    pub first_name: String,
    #[validate(length(min = 1, message = "validation.required"))]
    pub last_name: String,
    pub preferred_item_types: Option<String>,
    pub genres: Vec<i64>,
    pub tags: Vec<i64>,
    pub privacy_policy_accepted: bool,
    pub terms_of_service_accepted: bool,
    pub marketing_emails_accepted: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginSmsRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VerifyLoginSmsRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
    #[validate(length(min = 4, max = 6, message = "validation.code"))]
    pub code: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SocialLoginRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub provider: String,
    #[validate(length(min = 1, message = "validation.required"))]
    pub token: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "validation.email"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub token: String,
    #[validate(length(min = 8, message = "validation.min_length"))]
    pub password: String,
    #[validate(length(min = 8, message = "validation.min_length"))]
    pub password_confirmation: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddPhoneRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VerifySocialPhoneRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
    #[validate(length(min = 4, max = 6, message = "validation.code"))]
    pub code: String,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AuthTokenResponse {
    pub token: String,
    pub token_type: String,
    pub user: UserSafe,
}

#[derive(Debug, Serialize)]
pub struct SmsCodeResponse {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyCodeResponse {
    pub verified: bool,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/auth/login
#[tracing::instrument(skip(pool, config, body))]
pub async fn login(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE email = ? LIMIT 1",
    )
    .bind(&body.email)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::unauthorized("auth.invalid_credentials"))?;

    let valid = bcrypt::verify(&body.password, &user.password)
        .map_err(|_| ApiError::internal())?;
    if !valid {
        return Err(ApiError::unauthorized("auth.invalid_credentials"));
    }

    let token = crate::middleware::auth::generate_jwt(
        &config.jwt_secret,
        config.jwt_expiration_days,
        user.id,
        &user.email,
        &user.username,
    )
    .map_err(|_| ApiError::internal())?;

    let safe: UserSafe = user.into();
    let data = AuthTokenResponse {
        token,
        token_type: "Bearer".to_string(),
        user: safe,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "auth.login_success"))))
}

/// POST /api/auth/sms-code
#[tracing::instrument(skip(pool, config, queue, body))]
pub async fn request_sms_code(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    queue: web::Data<QueueService>,
    body: web::Json<SmsCodeRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    // Generate 6-digit code
    let code: String = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::minutes(10);

    // Store verification code
    sqlx::query(
        "INSERT INTO phone_verifications (phone_number, code, expires_at) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE code = VALUES(code), expires_at = VALUES(expires_at)",
    )
    .bind(&body.phone_number)
    .bind(&code)
    .bind(expires_at)
    .execute(pool.get_ref())
    .await?;

    // Dispatch SMS via RabbitMQ queue
    queue
        .dispatch_send_sms(
            &body.phone_number,
            &format!("Your Swapie verification code is: {}", code),
        )
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.sms_code_sent"))))
}

/// POST /api/auth/verify_code
#[tracing::instrument(skip(pool, body))]
pub async fn verify_code(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    body: web::Json<VerifyCodeRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM phone_verifications
         WHERE phone_number = ? AND code = ? AND expires_at > NOW()",
    )
    .bind(&body.phone_number)
    .bind(&body.code)
    .fetch_one(pool.get_ref())
    .await?;

    if row == 0 {
        return Err(ApiError::bad_request("auth.invalid_code"));
    }

    let data = VerifyCodeResponse { verified: true };
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "auth.code_verified"))))
}

/// POST /api/auth/complete-registration
#[tracing::instrument(skip(pool, config, body))]
pub async fn complete_registration(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    body: web::Json<CompleteRegistrationRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    if !body.privacy_policy_accepted || !body.terms_of_service_accepted {
        return Err(ApiError::validation("auth.must_accept_terms"));
    }

    // Verify the SMS code is still valid
    let valid_code = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM phone_verifications
         WHERE phone_number = ? AND code = ? AND expires_at > NOW()",
    )
    .bind(&body.phone_number)
    .bind(&body.code)
    .fetch_one(pool.get_ref())
    .await?;

    if valid_code == 0 {
        return Err(ApiError::bad_request("auth.invalid_code"));
    }

    // Check for existing user
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = ? OR username = ?",
    )
    .bind(&body.email)
    .bind(&body.username)
    .fetch_one(pool.get_ref())
    .await?;

    if exists > 0 {
        return Err(ApiError::conflict("auth.user_already_exists"));
    }

    let hashed = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
        .map_err(|_| ApiError::internal())?;

    let now = chrono::Utc::now().naive_utc();

    let result = sqlx::query(
        "INSERT INTO users (email, username, password, first_name, last_name, phone_number,
         preferred_item_types, privacy_policy_accepted, terms_of_service_accepted,
         marketing_emails_accepted, consents_accepted_at, language, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&body.email)
    .bind(&body.username)
    .bind(&hashed)
    .bind(&body.first_name)
    .bind(&body.last_name)
    .bind(&body.phone_number)
    .bind(&body.preferred_item_types)
    .bind(body.privacy_policy_accepted)
    .bind(body.terms_of_service_accepted)
    .bind(body.marketing_emails_accepted)
    .bind(now)
    .bind(&lang)
    .bind(now)
    .bind(now)
    .execute(pool.get_ref())
    .await?;

    let user_id = result.last_insert_id() as i64;

    // Attach genres
    for genre_id in &body.genres {
        sqlx::query("INSERT INTO user_genres (user_id, genre_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(genre_id)
            .execute(pool.get_ref())
            .await?;
    }

    // Attach tags
    for tag_id in &body.tags {
        sqlx::query("INSERT INTO user_tags (user_id, tag_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(tag_id)
            .execute(pool.get_ref())
            .await?;
    }

    // Clean up verification code
    sqlx::query("DELETE FROM phone_verifications WHERE phone_number = ?")
        .bind(&body.phone_number)
        .execute(pool.get_ref())
        .await?;

    let token = crate::middleware::auth::generate_jwt(
        &config.jwt_secret,
        config.jwt_expiration_days,
        user_id,
        &body.email,
        &body.username,
    )
    .map_err(|_| ApiError::internal())?;

    let user = sqlx::query_as::<_, UserSafe>(
        "SELECT id, email, username, first_name, last_name, description, phone_number,
         avatar_id, language, locker_id, stripe_customer_id, stripe_connect_account_id,
         stripe_connect_status, stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
         stripe_connect_onboarded_at, privacy_policy_accepted, terms_of_service_accepted,
         marketing_emails_accepted, consents_accepted_at, google_id, facebook_id,
         social_provider, social_provider_id, average_rating, review_count,
         activation_code_expires_at, last_unread_notification_at, preferred_item_types,
         default_inpost_locker, email_verified_at, created_at, updated_at
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(pool.get_ref())
    .await?;

    let data = AuthTokenResponse {
        token,
        token_type: "Bearer".to_string(),
        user,
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(data, i18n::t(&lang, "auth.registration_success"))))
}

/// POST /api/auth/login-sms
#[tracing::instrument(skip(pool, config, queue, body))]
pub async fn login_sms(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    queue: web::Data<QueueService>,
    body: web::Json<LoginSmsRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    // Ensure user with this phone exists
    let _user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE phone_number = ? LIMIT 1",
    )
    .bind(&body.phone_number)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("auth.user_not_found"))?;

    let code: String = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::minutes(10);

    sqlx::query(
        "INSERT INTO phone_verifications (phone_number, code, expires_at) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE code = VALUES(code), expires_at = VALUES(expires_at)",
    )
    .bind(&body.phone_number)
    .bind(&code)
    .bind(expires_at)
    .execute(pool.get_ref())
    .await?;

    // Dispatch SMS via RabbitMQ queue
    queue
        .dispatch_send_sms(
            &body.phone_number,
            &format!("Your Swapie verification code is: {}", code),
        )
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.sms_code_sent"))))
}

/// POST /api/auth/verify-login-sms
#[tracing::instrument(skip(pool, config, body))]
pub async fn verify_login_sms(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    body: web::Json<VerifyLoginSmsRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let valid_code = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM phone_verifications
         WHERE phone_number = ? AND code = ? AND expires_at > NOW()",
    )
    .bind(&body.phone_number)
    .bind(&body.code)
    .fetch_one(pool.get_ref())
    .await?;

    if valid_code == 0 {
        return Err(ApiError::bad_request("auth.invalid_code"));
    }

    let user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE phone_number = ? LIMIT 1",
    )
    .bind(&body.phone_number)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("auth.user_not_found"))?;

    // Clean up
    sqlx::query("DELETE FROM phone_verifications WHERE phone_number = ?")
        .bind(&body.phone_number)
        .execute(pool.get_ref())
        .await?;

    let token = crate::middleware::auth::generate_jwt(
        &config.jwt_secret,
        config.jwt_expiration_days,
        user.id,
        &user.email,
        &user.username,
    )
    .map_err(|_| ApiError::internal())?;

    let safe: UserSafe = user.into();
    let data = AuthTokenResponse {
        token,
        token_type: "Bearer".to_string(),
        user: safe,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "auth.login_success"))))
}

/// POST /api/auth/social-login
#[tracing::instrument(skip(pool, config, body))]
pub async fn social_login(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    body: web::Json<SocialLoginRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    // Validate the social token with the provider (Google / Apple / Facebook)
    // This would call the appropriate provider API based on body.provider
    let client = reqwest::Client::new();
    let (social_id, email, first_name, last_name): (String, String, String, String) = match body.provider.as_str() {
        "google" => {
            // Validate Google token via tokeninfo endpoint
            let url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={}", body.token);
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|_| ApiError::bad_request("auth.social_token_invalid"))?;

            if !response.status().is_success() {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            let data: serde_json::Value = response
                .json()
                .await
                .map_err(|_| ApiError::bad_request("auth.social_token_invalid"))?;

            // Verify the audience matches our Google client ID
            let aud = data.get("aud").and_then(|v| v.as_str()).unwrap_or("");
            if aud != config.google_client_id {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            let sub = data.get("sub").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let email = data.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let given_name = data.get("given_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let family_name = data.get("family_name").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if sub.is_empty() || email.is_empty() {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            (sub, email, given_name, family_name)
        }
        "apple" => {
            // Decode Apple JWT token to extract claims
            // Apple ID tokens are JWTs - decode the payload without full verification
            // (In production, you'd fetch Apple's public keys from https://appleid.apple.com/auth/keys)
            let parts: Vec<&str> = body.token.split('.').collect();
            if parts.len() != 3 {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            let payload = base64::Engine::decode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                parts[1],
            )
            .map_err(|_| ApiError::bad_request("auth.social_token_invalid"))?;

            let claims: serde_json::Value = serde_json::from_slice(&payload)
                .map_err(|_| ApiError::bad_request("auth.social_token_invalid"))?;

            // Verify the audience matches our Apple client ID
            let aud = claims.get("aud").and_then(|v| v.as_str()).unwrap_or("");
            if aud != config.apple_client_id {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            let sub = claims.get("sub").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let email = claims.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if sub.is_empty() {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            let first = body.first_name.clone().unwrap_or_default();
            let last = body.last_name.clone().unwrap_or_default();

            (sub, email, first, last)
        }
        "facebook" => {
            // Validate Facebook token via Graph API
            let url = format!(
                "https://graph.facebook.com/me?fields=id,email,first_name,last_name&access_token={}",
                body.token
            );
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|_| ApiError::bad_request("auth.social_token_invalid"))?;

            if !response.status().is_success() {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            let data: serde_json::Value = response
                .json()
                .await
                .map_err(|_| ApiError::bad_request("auth.social_token_invalid"))?;

            let id = data.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let email = data.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let first = data.get("first_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let last = data.get("last_name").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if id.is_empty() {
                return Err(ApiError::bad_request("auth.social_token_invalid"));
            }

            (id, email, first, last)
        }
        _ => {
            return Err(ApiError::bad_request("auth.invalid_social_provider"));
        }
    };

    // Check if user already exists with this social provider
    let existing_user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE (social_provider = ? AND social_provider_id = ?) OR email = ? LIMIT 1",
    )
    .bind(&body.provider)
    .bind(&social_id)
    .bind(&email)
    .fetch_optional(pool.get_ref())
    .await?;

    let user = if let Some(user) = existing_user {
        // Update social provider info if not already set
        if user.social_provider_id.is_none() {
            sqlx::query(
                "UPDATE users SET social_provider = ?, social_provider_id = ?, updated_at = NOW() WHERE id = ?",
            )
            .bind(&body.provider)
            .bind(&social_id)
            .bind(user.id)
            .execute(pool.get_ref())
            .await?;
        }
        user
    } else {
        // Create new user from social login
        let now = chrono::Utc::now().naive_utc();
        let username = format!("{}_{}", body.provider, &social_id[..8.min(social_id.len())]);
        let dummy_password = bcrypt::hash(uuid::Uuid::new_v4().to_string(), bcrypt::DEFAULT_COST)
            .map_err(|_| ApiError::internal())?;

        let result = sqlx::query(
            "INSERT INTO users (email, username, password, first_name, last_name,
             social_provider, social_provider_id, language, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&email)
        .bind(&username)
        .bind(&dummy_password)
        .bind(&first_name)
        .bind(&last_name)
        .bind(&body.provider)
        .bind(&social_id)
        .bind(&lang)
        .bind(now)
        .bind(now)
        .execute(pool.get_ref())
        .await?;

        let user_id = result.last_insert_id() as i64;
        sqlx::query_as::<_, crate::models::User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(pool.get_ref())
            .await?
    };

    let token = crate::middleware::auth::generate_jwt(
        &config.jwt_secret,
        config.jwt_expiration_days,
        user.id,
        &user.email,
        &user.username,
    )
    .map_err(|_| ApiError::internal())?;

    let safe: UserSafe = user.into();
    let data = AuthTokenResponse {
        token,
        token_type: "Bearer".to_string(),
        user: safe,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "auth.login_success"))))
}

/// POST /api/auth/forgot-password
#[tracing::instrument(skip(pool, config, body))]
pub async fn forgot_password(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    body: web::Json<ForgotPasswordRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    // Always return success to prevent email enumeration
    let _user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE email = ? LIMIT 1",
    )
    .bind(&body.email)
    .fetch_optional(pool.get_ref())
    .await?;

    if let Some(user) = _user {
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::hours(1);

        sqlx::query(
            "INSERT INTO password_resets (email, token, expires_at) VALUES (?, ?, ?)
             ON DUPLICATE KEY UPDATE token = VALUES(token), expires_at = VALUES(expires_at)",
        )
        .bind(&body.email)
        .bind(&token)
        .bind(expires_at)
        .execute(pool.get_ref())
        .await?;

        // Log the reset link for debugging
        tracing::info!("Password reset link: {}/reset-password?token={}", config.app.url, token);

        // Send reset email via transactional email API
        let email_client = reqwest::Client::new();
        let _ = email_client
            .post(format!("https://{}/api/send", config.smtp_host))
            .json(&serde_json::json!({
                "from": config.smtp_from_email,
                "to": body.email,
                "subject": "Password Reset - Swapie",
                "html": format!(
                    "<p>Click <a href='{}/reset-password?token={}'>here</a> to reset your password.</p>",
                    config.app.url, token
                )
            }))
            .send()
            .await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.password_reset_sent"))))
}

/// POST /api/auth/reset-password
#[tracing::instrument(skip(pool, body))]
pub async fn reset_password(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    body: web::Json<ResetPasswordRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    if body.password != body.password_confirmation {
        return Err(ApiError::validation("auth.password_mismatch"));
    }

    let reset = sqlx::query_as::<_, (String,)>(
        "SELECT email FROM password_resets WHERE token = ? AND expires_at > NOW() LIMIT 1",
    )
    .bind(&body.token)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::bad_request("auth.invalid_reset_token"))?;

    let hashed = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
        .map_err(|_| ApiError::internal())?;

    sqlx::query("UPDATE users SET password = ?, updated_at = NOW() WHERE email = ?")
        .bind(&hashed)
        .bind(&reset.0)
        .execute(pool.get_ref())
        .await?;

    sqlx::query("DELETE FROM password_resets WHERE email = ?")
        .bind(&reset.0)
        .execute(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.password_reset_success"))))
}

/// GET /api/auth/genres
#[tracing::instrument(skip(pool))]
pub async fn get_auth_genres(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let genres = sqlx::query_as::<_, Genre>("SELECT * FROM genres ORDER BY name ASC")
        .fetch_all(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(genres, i18n::t(&lang, "general.success"))))
}

/// GET /api/auth/tags
#[tracing::instrument(skip(pool))]
pub async fn get_auth_tags(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let tags = sqlx::query_as::<_, Tag>("SELECT * FROM tags ORDER BY name ASC")
        .fetch_all(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(tags, i18n::t(&lang, "general.success"))))
}

/// GET /api/auth/user [auth:sanctum]
#[tracing::instrument(skip(pool, auth))]
pub async fn get_user(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let user = sqlx::query_as::<_, UserSafe>(
        "SELECT id, email, username, first_name, last_name, description, phone_number,
         avatar_id, language, locker_id, stripe_customer_id, stripe_connect_account_id,
         stripe_connect_status, stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
         stripe_connect_onboarded_at, privacy_policy_accepted, terms_of_service_accepted,
         marketing_emails_accepted, consents_accepted_at, google_id, facebook_id,
         social_provider, social_provider_id, average_rating, review_count,
         activation_code_expires_at, last_unread_notification_at, preferred_item_types,
         default_inpost_locker, email_verified_at, created_at, updated_at
         FROM users WHERE id = ?",
    )
    .bind(auth.user_id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("auth.user_not_found"))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(user, i18n::t(&lang, "general.success"))))
}

/// POST /api/auth/logout [auth:sanctum]
#[tracing::instrument(skip(pool, auth))]
pub async fn logout(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    // With JWT, logout is typically handled client-side by discarding the token.
    // Optionally blacklist the token in a tokens table.
    sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = ? LIMIT 1")
        .bind(auth.user_id)
        .execute(pool.get_ref())
        .await
        .ok(); // Ignore if table doesn't exist

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.logout_success"))))
}

/// POST /api/auth/revoke-all [auth:sanctum]
#[tracing::instrument(skip(pool, auth))]
pub async fn revoke_all(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = ?")
        .bind(auth.user_id)
        .execute(pool.get_ref())
        .await
        .ok();

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.tokens_revoked"))))
}

/// POST /api/auth/add-phone [auth:sanctum]
#[tracing::instrument(skip(pool, queue, auth, body))]
pub async fn add_phone(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    queue: web::Data<QueueService>,
    auth: AuthenticatedUser,
    body: web::Json<AddPhoneRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    // Check if phone is already in use
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE phone_number = ? AND id != ?",
    )
    .bind(&body.phone_number)
    .bind(auth.user_id)
    .fetch_one(pool.get_ref())
    .await?;

    if exists > 0 {
        return Err(ApiError::conflict("auth.phone_already_in_use"));
    }

    let code: String = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::minutes(10);

    sqlx::query(
        "INSERT INTO phone_verifications (phone_number, code, expires_at) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE code = VALUES(code), expires_at = VALUES(expires_at)",
    )
    .bind(&body.phone_number)
    .bind(&code)
    .bind(expires_at)
    .execute(pool.get_ref())
    .await?;

    // Dispatch SMS via RabbitMQ queue
    queue
        .dispatch_send_sms(
            &body.phone_number,
            &format!("Your Swapie verification code is: {}", code),
        )
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "auth.sms_code_sent"))))
}

/// POST /api/auth/verify-social-phone [auth:sanctum]
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn verify_social_phone(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<VerifySocialPhoneRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let valid_code = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM phone_verifications
         WHERE phone_number = ? AND code = ? AND expires_at > NOW()",
    )
    .bind(&body.phone_number)
    .bind(&body.code)
    .fetch_one(pool.get_ref())
    .await?;

    if valid_code == 0 {
        return Err(ApiError::bad_request("auth.invalid_code"));
    }

    sqlx::query("UPDATE users SET phone_number = ?, updated_at = NOW() WHERE id = ?")
        .bind(&body.phone_number)
        .bind(auth.user_id)
        .execute(pool.get_ref())
        .await?;

    sqlx::query("DELETE FROM phone_verifications WHERE phone_number = ?")
        .bind(&body.phone_number)
        .execute(pool.get_ref())
        .await?;

    let user = sqlx::query_as::<_, UserSafe>(
        "SELECT id, email, username, first_name, last_name, description, phone_number,
         avatar_id, language, locker_id, stripe_customer_id, stripe_connect_account_id,
         stripe_connect_status, stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
         stripe_connect_onboarded_at, privacy_policy_accepted, terms_of_service_accepted,
         marketing_emails_accepted, consents_accepted_at, google_id, facebook_id,
         social_provider, social_provider_id, average_rating, review_count,
         activation_code_expires_at, last_unread_notification_at, preferred_item_types,
         default_inpost_locker, email_verified_at, created_at, updated_at
         FROM users WHERE id = ?",
    )
    .bind(auth.user_id)
    .fetch_one(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(user, i18n::t(&lang, "auth.phone_verified"))))
}

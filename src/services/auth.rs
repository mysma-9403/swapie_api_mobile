use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::config::Config;
use crate::errors::ApiError;
use crate::middleware::auth::{generate_jwt, verify_jwt, Claims};
use crate::models::User;

// ── DTOs ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterData {
    pub email: String,
    pub username: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: i64,
    pub email: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct SocialLoginData {
    pub provider: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AppleUserInfo {
    pub sub: String,
    pub email: Option<String>,
}

// ── Auth Service ────────────────────────────────────────────────────────

pub struct AuthService;

impl AuthService {
    /// Register a new user with hashed password.
    pub async fn register(pool: &MySqlPool, data: RegisterData) -> Result<AuthResponse, ApiError> {
        // Check if email already exists.
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users WHERE email = ?")
                .bind(&data.email)
                .fetch_optional(pool)
                .await?;

        if existing.is_some() {
            return Err(ApiError::conflict("auth.email_taken"));
        }

        // Check if username already exists.
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(&data.username)
                .fetch_optional(pool)
                .await?;

        if existing.is_some() {
            return Err(ApiError::conflict("auth.username_taken"));
        }

        let hashed = Self::hash_password(&data.password)?;

        let result = sqlx::query(
            r#"
            INSERT INTO users (email, username, password, first_name, last_name, language, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 'pl', NOW(), NOW())
            "#,
        )
        .bind(&data.email)
        .bind(&data.username)
        .bind(&hashed)
        .bind(&data.first_name)
        .bind(&data.last_name)
        .execute(pool)
        .await?;

        let user_id = result.last_insert_id() as i64;

        Ok(AuthResponse {
            token: String::new(), // Caller should generate JWT after this.
            user_id,
            email: data.email,
            username: data.username,
        })
    }

    /// Validate credentials and return an auth response with JWT.
    pub async fn login(
        pool: &MySqlPool,
        config: &Config,
        email: &str,
        password: &str,
    ) -> Result<AuthResponse, ApiError> {
        let user: User = sqlx::query_as(
            "SELECT * FROM users WHERE email = ?",
        )
        .bind(email)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::unauthorized("auth.invalid_credentials"))?;

        if !Self::verify_password(password, &user.password)? {
            return Err(ApiError::unauthorized("auth.invalid_credentials"));
        }

        let token = Self::generate_token(config, &user)?;

        Ok(AuthResponse {
            token,
            user_id: user.id,
            email: user.email,
            username: user.username,
        })
    }

    /// Generate a JWT token for a user.
    pub fn generate_token(config: &Config, user: &User) -> Result<String, ApiError> {
        generate_jwt(
            &config.jwt_secret,
            config.jwt_expiration_days,
            user.id,
            &user.email,
            &user.username,
        )
        .map_err(|e| {
            tracing::error!("Failed to generate JWT: {:?}", e);
            ApiError::internal()
        })
    }

    /// Verify a JWT token and return the claims.
    pub fn verify_token(config: &Config, token: &str) -> Result<Claims, ApiError> {
        verify_jwt(&config.jwt_secret, token).map_err(|e| {
            tracing::warn!("JWT verification failed: {:?}", e);
            ApiError::unauthorized("auth.invalid_token")
        })
    }

    /// Send an SMS verification code. Stores the code in the database with an
    /// expiry time.
    pub async fn send_sms_code(
        config: &Config,
        pool: &MySqlPool,
        phone: &str,
    ) -> Result<(), ApiError> {
        let code = Self::generate_numeric_code(6);
        let expires_at = Utc::now().naive_utc() + chrono::Duration::minutes(10);

        // Upsert the verification code.
        sqlx::query(
            r#"
            INSERT INTO sms_verifications (phone_number, code, expires_at, created_at)
            VALUES (?, ?, ?, NOW())
            ON DUPLICATE KEY UPDATE code = VALUES(code), expires_at = VALUES(expires_at)
            "#,
        )
        .bind(phone)
        .bind(&code)
        .bind(expires_at)
        .execute(pool)
        .await?;

        // Send the SMS via external API.
        let client = reqwest::Client::new();
        let _response = client
            .post(&config.sms_api_url)
            .header("Authorization", format!("Bearer {}", config.sms_api_key))
            .json(&serde_json::json!({
                "phone": phone,
                "message": format!("Your Swapie verification code is: {}", code),
            }))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to send SMS: {:?}", e);
                ApiError::external_service("auth.sms_send_failed")
            })?;

        Ok(())
    }

    /// Verify an SMS code against the stored value.
    pub async fn verify_sms_code(
        pool: &MySqlPool,
        phone: &str,
        code: &str,
    ) -> Result<bool, ApiError> {
        let record: Option<(String, NaiveDateTime)> = sqlx::query_as(
            "SELECT code, expires_at FROM sms_verifications WHERE phone_number = ?",
        )
        .bind(phone)
        .fetch_optional(pool)
        .await?;

        match record {
            Some((stored_code, expires_at)) => {
                if Utc::now().naive_utc() > expires_at {
                    return Ok(false);
                }
                if stored_code != code {
                    return Ok(false);
                }
                // Delete the used code.
                sqlx::query("DELETE FROM sms_verifications WHERE phone_number = ?")
                    .bind(phone)
                    .execute(pool)
                    .await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Handle social login (Google or Apple). Creates or finds the user.
    pub async fn social_login(
        pool: &MySqlPool,
        config: &Config,
        provider: &str,
        token: &str,
    ) -> Result<AuthResponse, ApiError> {
        match provider {
            "google" => Self::google_login(pool, config, token).await,
            "apple" => Self::apple_login(pool, config, token).await,
            _ => Err(ApiError::bad_request("auth.unsupported_provider")),
        }
    }

    /// Generate a password reset token and return it.
    pub async fn reset_password(pool: &MySqlPool, email: &str) -> Result<String, ApiError> {
        let user: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users WHERE email = ?")
                .bind(email)
                .fetch_optional(pool)
                .await?;

        let (user_id,) = user.ok_or_else(|| ApiError::not_found("auth.user_not_found"))?;

        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now().naive_utc() + chrono::Duration::hours(1);

        sqlx::query(
            r#"
            INSERT INTO password_resets (user_id, token, expires_at, created_at)
            VALUES (?, ?, ?, NOW())
            "#,
        )
        .bind(user_id)
        .bind(&token)
        .bind(expires_at)
        .execute(pool)
        .await?;

        Ok(token)
    }

    /// Complete the password reset by validating the token and updating the password.
    pub async fn complete_reset_password(
        pool: &MySqlPool,
        token: &str,
        new_password: &str,
    ) -> Result<(), ApiError> {
        let record: Option<(i64, NaiveDateTime)> = sqlx::query_as(
            "SELECT user_id, expires_at FROM password_resets WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        let (user_id, expires_at) =
            record.ok_or_else(|| ApiError::bad_request("auth.invalid_reset_token"))?;

        if Utc::now().naive_utc() > expires_at {
            return Err(ApiError::bad_request("auth.reset_token_expired"));
        }

        let hashed = Self::hash_password(new_password)?;

        sqlx::query("UPDATE users SET password = ?, updated_at = NOW() WHERE id = ?")
            .bind(&hashed)
            .bind(user_id)
            .execute(pool)
            .await?;

        // Delete used reset token.
        sqlx::query("DELETE FROM password_resets WHERE token = ?")
            .bind(token)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Hash a password using Argon2.
    pub fn hash_password(password: &str) -> Result<String, ApiError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                tracing::error!("Password hashing failed: {:?}", e);
                ApiError::internal()
            })?;
        Ok(hash.to_string())
    }

    /// Verify a password against an Argon2 hash.
    pub fn verify_password(password: &str, hash: &str) -> Result<bool, ApiError> {
        let parsed_hash = PasswordHash::new(hash).map_err(|e| {
            tracing::error!("Failed to parse password hash: {:?}", e);
            ApiError::internal()
        })?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    // ── Private helpers ─────────────────────────────────────────────────

    async fn google_login(
        pool: &MySqlPool,
        config: &Config,
        token: &str,
    ) -> Result<AuthResponse, ApiError> {
        // Validate token with Google.
        let client = reqwest::Client::new();
        let user_info: GoogleUserInfo = client
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Google API request failed: {:?}", e);
                ApiError::external_service("auth.social_login_failed")
            })?
            .json()
            .await
            .map_err(|e| {
                tracing::error!("Failed to parse Google user info: {:?}", e);
                ApiError::external_service("auth.social_login_failed")
            })?;

        Self::find_or_create_social_user(
            pool,
            config,
            "google",
            &user_info.sub,
            &user_info.email,
            user_info.given_name.as_deref().unwrap_or(""),
            user_info.family_name.as_deref().unwrap_or(""),
        )
        .await
    }

    async fn apple_login(
        pool: &MySqlPool,
        config: &Config,
        token: &str,
    ) -> Result<AuthResponse, ApiError> {
        // Validate the Apple identity token by decoding it (simplified).
        // In production, you would verify the token signature against Apple's public keys.
        let client = reqwest::Client::new();
        let response = client
            .post("https://appleid.apple.com/auth/token")
            .form(&[
                ("client_id", config.apple_client_id.as_str()),
                ("client_secret", ""), // Generated from Apple private key
                ("code", token),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Apple API request failed: {:?}", e);
                ApiError::external_service("auth.social_login_failed")
            })?;

        let apple_response: serde_json::Value = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse Apple response: {:?}", e);
            ApiError::external_service("auth.social_login_failed")
        })?;

        // Extract user info from the id_token.
        let id_token = apple_response["id_token"]
            .as_str()
            .ok_or_else(|| ApiError::external_service("auth.social_login_failed"))?;

        // Decode the JWT payload (without verification for the sub extraction;
        // full verification should use Apple's public keys in production).
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(ApiError::external_service("auth.social_login_failed"));
        }

        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| ApiError::external_service("auth.social_login_failed"))?;

        let claims: AppleUserInfo = serde_json::from_slice(&payload)
            .map_err(|_| ApiError::external_service("auth.social_login_failed"))?;

        let email = claims
            .email
            .unwrap_or_else(|| format!("{}@privaterelay.appleid.com", claims.sub));

        Self::find_or_create_social_user(pool, config, "apple", &claims.sub, &email, "", "").await
    }

    async fn find_or_create_social_user(
        pool: &MySqlPool,
        config: &Config,
        provider: &str,
        provider_id: &str,
        email: &str,
        first_name: &str,
        last_name: &str,
    ) -> Result<AuthResponse, ApiError> {
        // Try to find existing user by social provider ID.
        let existing: Option<User> = sqlx::query_as(
            "SELECT * FROM users WHERE social_provider = ? AND social_provider_id = ?",
        )
        .bind(provider)
        .bind(provider_id)
        .fetch_optional(pool)
        .await?;

        if let Some(user) = existing {
            let token = Self::generate_token(config, &user)?;
            return Ok(AuthResponse {
                token,
                user_id: user.id,
                email: user.email,
                username: user.username,
            });
        }

        // Try to find existing user by email.
        let existing_by_email: Option<User> =
            sqlx::query_as("SELECT * FROM users WHERE email = ?")
                .bind(email)
                .fetch_optional(pool)
                .await?;

        if let Some(user) = existing_by_email {
            // Link the social account to the existing user.
            sqlx::query(
                "UPDATE users SET social_provider = ?, social_provider_id = ?, updated_at = NOW() WHERE id = ?",
            )
            .bind(provider)
            .bind(provider_id)
            .bind(user.id)
            .execute(pool)
            .await?;

            let token = Self::generate_token(config, &user)?;
            return Ok(AuthResponse {
                token,
                user_id: user.id,
                email: user.email,
                username: user.username,
            });
        }

        // Create a new user.
        let username = Self::generate_unique_username(pool, email).await?;
        let dummy_password = uuid::Uuid::new_v4().to_string();
        let hashed = Self::hash_password(&dummy_password)?;

        let result = sqlx::query(
            r#"
            INSERT INTO users (email, username, password, first_name, last_name, social_provider, social_provider_id, language, email_verified_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, 'pl', NOW(), NOW(), NOW())
            "#,
        )
        .bind(email)
        .bind(&username)
        .bind(&hashed)
        .bind(first_name)
        .bind(last_name)
        .bind(provider)
        .bind(provider_id)
        .execute(pool)
        .await?;

        let user_id = result.last_insert_id() as i64;

        let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

        let token = Self::generate_token(config, &user)?;

        Ok(AuthResponse {
            token,
            user_id: user.id,
            email: user.email,
            username: user.username,
        })
    }

    async fn generate_unique_username(pool: &MySqlPool, email: &str) -> Result<String, ApiError> {
        let base = email.split('@').next().unwrap_or("user");
        let mut username = base.to_string();
        let mut counter = 1u32;

        loop {
            let exists: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM users WHERE username = ?")
                    .bind(&username)
                    .fetch_optional(pool)
                    .await?;

            if exists.is_none() {
                return Ok(username);
            }

            username = format!("{}{}", base, counter);
            counter += 1;
        }
    }

    fn generate_numeric_code(length: usize) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| rng.gen_range(0..10).to_string())
            .collect()
    }
}

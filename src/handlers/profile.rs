use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::ProfileService;
use crate::services::profile::{UpdateGdprConsentData, UpdateLockerData, UpdateProfileData};

// ── Helper ───────────────────────────────────────────────────────────────────

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
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, max = 100, message = "validation.name_length"))]
    pub first_name: Option<String>,
    #[validate(length(min = 1, max = 100, message = "validation.name_length"))]
    pub last_name: Option<String>,
    #[validate(length(max = 1000, message = "validation.description_length"))]
    pub description: Option<String>,
    pub language: Option<String>,
    pub preferred_item_types: Option<String>,
    pub genres: Option<Vec<i64>>,
    pub tags: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAddressRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub street: String,
    #[validate(length(min = 1, message = "validation.required"))]
    pub building_number: String,
    pub flat_number: Option<String>,
    #[validate(length(min = 1, message = "validation.required"))]
    pub zip_code: String,
    #[validate(length(min = 1, message = "validation.required"))]
    pub city: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePhoneRequest {
    #[validate(length(min = 9, message = "validation.phone"))]
    pub phone_number: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLockerRequest {
    pub locker_name: String,
    pub provider: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct GdprConsentRequest {
    pub privacy_policy: bool,
    pub terms_of_service: bool,
    pub marketing_emails: bool,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PublicProfileResponse {
    pub user: PublicUserInfo,
    pub books: Vec<crate::models::Book>,
    pub review_count: i32,
    pub average_rating: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Serialize)]
pub struct PublicUserInfo {
    pub id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub description: Option<String>,
    pub avatar_id: Option<i64>,
    pub average_rating: Option<rust_decimal::Decimal>,
    pub review_count: i32,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct GdprConsentResponse {
    pub privacy_policy: bool,
    pub terms_of_service: bool,
    pub marketing_emails: bool,
    pub consents_accepted_at: Option<chrono::NaiveDateTime>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/profile
#[tracing::instrument(skip(pool, auth))]
pub async fn get_profile(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let profile = ProfileService::get_profile(pool.get_ref(), auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(profile, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/profile/details
#[tracing::instrument(skip(pool, auth))]
pub async fn get_profile_details(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let details = ProfileService::get_profile_details(pool.get_ref(), auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(details, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/profile/user/{userId}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_user_profile(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let user_id = path.into_inner();

    let profile = ProfileService::get_user_profile_with_books(pool.get_ref(), user_id).await?;

    let data = PublicProfileResponse {
        review_count: profile.review_count,
        average_rating: profile.average_rating,
        user: PublicUserInfo {
            id: profile.user_id,
            username: profile.username,
            first_name: profile.first_name,
            last_name: profile.last_name,
            description: profile.description,
            avatar_id: profile.avatar_id,
            average_rating: profile.average_rating,
            review_count: profile.review_count,
            created_at: profile.created_at,
        },
        books: profile.books,
    };
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// PUT /api/v1/profile/update
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_profile(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateProfileRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let data = UpdateProfileData {
        first_name: body.first_name.clone(),
        last_name: body.last_name.clone(),
        username: None,
        description: body.description.clone(),
        language: body.language.clone(),
        preferred_item_types: body.preferred_item_types.clone(),
    };

    let user = ProfileService::update_profile_with_relations(
        pool.get_ref(),
        auth.user_id,
        data,
        body.genres.clone(),
        body.tags.clone(),
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(user, i18n::t(&lang, "profile.updated"))))
}

/// PUT /api/v1/profile/address
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_address(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateAddressRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let data = crate::services::profile::UpdateAddressData {
        street: Some(body.street.clone()),
        city: Some(body.city.clone()),
        postal_code: Some(body.zip_code.clone()),
        country: None,
        state: None,
        latitude: None,
        longitude: None,
    };

    let address = ProfileService::update_address(pool.get_ref(), auth.user_id, data).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(address, i18n::t(&lang, "profile.address_updated"))))
}

/// PUT /api/v1/profile/phone
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_phone(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdatePhoneRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    ProfileService::update_phone_checked(pool.get_ref(), auth.user_id, &body.phone_number).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "profile.phone_updated"))))
}

/// PUT /api/v1/profile/locker
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_locker(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateLockerRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let data = UpdateLockerData {
        default_inpost_locker: Some(body.locker_name.clone()),
        locker_id: None,
    };

    ProfileService::update_locker(pool.get_ref(), auth.user_id, data).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "profile.locker_updated"))))
}

/// GET /api/v1/profile/export-data
#[tracing::instrument(skip(pool, auth))]
pub async fn export_data(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let data = ProfileService::export_data(pool.get_ref(), auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "profile.data_exported"))))
}

/// DELETE /api/v1/profile/account
#[tracing::instrument(skip(pool, auth))]
pub async fn delete_account(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let has_active = ProfileService::check_active_trades(pool.get_ref(), auth.user_id).await?;
    if has_active {
        return Err(ApiError::conflict("profile.cannot_delete_with_active_trades"));
    }

    ProfileService::delete_account(pool.get_ref(), auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "profile.account_deleted"))))
}

/// GET /api/v1/profile/gdpr-consent
#[tracing::instrument(skip(pool, auth))]
pub async fn get_gdpr_consent(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let consent = ProfileService::get_gdpr_consent(pool.get_ref(), auth.user_id).await?;

    let data = GdprConsentResponse {
        privacy_policy: consent.privacy_policy_accepted,
        terms_of_service: consent.terms_of_service_accepted,
        marketing_emails: consent.marketing_emails_accepted,
        consents_accepted_at: consent.consents_accepted_at,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// PUT /api/v1/profile/gdpr-consent
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_gdpr_consent(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<GdprConsentRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let data = UpdateGdprConsentData {
        privacy_policy_accepted: Some(body.privacy_policy),
        terms_of_service_accepted: Some(body.terms_of_service),
        marketing_emails_accepted: Some(body.marketing_emails),
    };

    ProfileService::update_gdpr_consent(pool.get_ref(), auth.user_id, data).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "profile.consent_updated"))))
}

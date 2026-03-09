use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::{ApiResponse, PaginatedResponse, PaginationParams};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
// Models used indirectly through service layer
use crate::services::NotificationService;

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
pub struct RegisterDeviceRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub fcm_token: String,
    #[validate(custom(function = "validate_device_type"))]
    pub device_type: String,
}

fn validate_device_type(device_type: &str) -> Result<(), validator::ValidationError> {
    match device_type {
        "ios" | "android" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid_device_type")),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UnregisterDeviceRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub fcm_token: String,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct UnreadCountResponse {
    pub count: i64,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/notifications
#[tracing::instrument(skip(pool, auth, query))]
pub async fn list_notifications(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let _lang = lang_from_req(&req);

    let total = NotificationService::get_total_count(pool.get_ref(), auth.user_id).await?;
    let notifications = NotificationService::list_notifications(
        pool.get_ref(),
        auth.user_id,
        query.per_page() as u64,
        query.offset(),
    )
    .await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(
        notifications,
        query.page(),
        query.per_page(),
        total,
    )))
}

/// GET /api/v1/notifications/unread-count
#[tracing::instrument(skip(pool, auth))]
pub async fn unread_count(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let count = NotificationService::get_unread_count(pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(
        UnreadCountResponse { count },
        i18n::t(&lang, "general.success"),
    )))
}

/// GET /api/v1/notifications/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_notification(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let notification_id = path.into_inner();

    let notification = NotificationService::get_notification(
        pool.get_ref(),
        notification_id,
        auth.user_id,
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(notification, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/notifications/{id}/read
#[tracing::instrument(skip(pool, auth))]
pub async fn mark_read(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let notification_id = path.into_inner();

    let updated = NotificationService::mark_as_read(pool.get_ref(), notification_id, auth.user_id).await?;

    if !updated {
        return Err(ApiError::not_found("notifications.not_found"));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "notifications.marked_read"))))
}

/// POST /api/v1/notifications/read-all
#[tracing::instrument(skip(pool, auth))]
pub async fn mark_all_read(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    NotificationService::mark_all_as_read(pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "notifications.all_marked_read"))))
}

/// POST /api/v1/device/token
#[tracing::instrument(skip(pool, auth, body))]
pub async fn register_device(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<RegisterDeviceRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    NotificationService::register_device(
        pool.get_ref(),
        auth.user_id,
        &body.fcm_token,
        &body.device_type,
    )
    .await?;

    Ok(HttpResponse::Created().json(ApiResponse::message(i18n::t(&lang, "notifications.device_registered"))))
}

/// DELETE /api/v1/device/token
#[tracing::instrument(skip(pool, auth, body))]
pub async fn unregister_device(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UnregisterDeviceRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    NotificationService::unregister_device(pool.get_ref(), auth.user_id, &body.fcm_token).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "notifications.device_unregistered"))))
}

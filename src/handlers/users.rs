use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
// Models used indirectly through service layer
use crate::services::ProfileService;

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
pub struct BlockUserRequest {
    pub user_id: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UnblockUserRequest {
    pub user_id: i64,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BlockedUserInfo {
    pub block_id: i64,
    pub user_id: i64,
    pub username: String,
    pub first_name: String,
    pub avatar_id: Option<i64>,
    pub reason: Option<String>,
    pub blocked_at: chrono::NaiveDateTime,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/users/block
#[tracing::instrument(skip(pool, auth, body))]
pub async fn block_user(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<BlockUserRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    ProfileService::block_user(
        pool.get_ref(),
        auth.user_id,
        body.user_id,
        body.reason.as_deref(),
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "users.blocked"))))
}

/// POST /api/v1/users/unblock
#[tracing::instrument(skip(pool, auth, body))]
pub async fn unblock_user(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UnblockUserRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    ProfileService::unblock_user(pool.get_ref(), auth.user_id, body.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "users.unblocked"))))
}

/// GET /api/v1/users/blocked
#[tracing::instrument(skip(pool, auth))]
pub async fn get_blocked_users(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let blocked_users = ProfileService::get_blocked_users(pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(blocked_users, i18n::t(&lang, "general.success"))))
}

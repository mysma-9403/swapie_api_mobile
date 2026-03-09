use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::{ApiResponse, PaginationParams};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::ReviewService;

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
pub struct CreateReviewRequest {
    #[validate(range(min = 1, max = 5, message = "validation.rating_range"))]
    pub rating: u8,
    #[validate(length(max = 2000, message = "validation.comment_length"))]
    pub comment: Option<String>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ReviewStatusResponse {
    pub can_review: bool,
    pub has_reviewed: bool,
    pub other_has_reviewed: bool,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/trades/{tradeId}/review
#[tracing::instrument(skip(pool, auth, body))]
pub async fn create_review(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<CreateReviewRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let review = ReviewService::create_review(
        pool.get_ref(),
        auth.user_id,
        trade_id,
        body.rating as i8,
        body.comment.as_deref(),
    ).await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(review, i18n::t(&lang, "reviews.created"))))
}

/// GET /api/v1/trades/{tradeId}/review-status
#[tracing::instrument(skip(pool, auth))]
pub async fn get_review_status(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let status = ReviewService::get_review_status_full(pool.get_ref(), trade_id, auth.user_id).await?;

    let data = ReviewStatusResponse {
        can_review: status.can_review,
        has_reviewed: status.has_reviewed,
        other_has_reviewed: status.other_has_reviewed,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/users/{userId}/reviews
#[tracing::instrument(skip(pool, auth, query))]
pub async fn get_user_reviews(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let _lang = lang_from_req(&req);
    let user_id = path.into_inner();

    let result = ReviewService::get_user_reviews_enriched(pool.get_ref(), user_id, &query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(result))
}

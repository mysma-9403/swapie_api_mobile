use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::SwipeService;

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
pub struct SwipeRequest {
    #[validate(custom(function = "validate_swipe_action"))]
    pub action: String,
}

fn validate_swipe_action(action: &str) -> Result<(), validator::ValidationError> {
    match action {
        "like" | "superlike" | "reject" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid_swipe_action")),
    }
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SwipeResult {
    pub matched: bool,
    pub match_id: Option<i64>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/swipe
#[tracing::instrument(skip(pool, auth))]
pub async fn get_next_swipe(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let candidate = SwipeService::get_next_swipe_detail(pool.get_ref(), auth.user_id).await?;

    match candidate {
        Some(data) => {
            Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
        }
        None => {
            Ok(HttpResponse::Ok().json(ApiResponse::<()>::message(i18n::t(&lang, "swipe.no_more_books"))))
        }
    }
}

/// POST /api/v1/swipe/{book_id}
#[tracing::instrument(skip(pool, auth, body))]
pub async fn handle_swipe(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<SwipeRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let result = SwipeService::handle_swipe(pool.get_ref(), auth.user_id, book_id, &body.action).await?;

    let matched = result.is_match;
    let match_id = result.match_record.as_ref().map(|m| m.id);

    let data = SwipeResult { matched, match_id };
    let message = if matched {
        i18n::t(&lang, "swipe.matched")
    } else {
        i18n::t(&lang, "swipe.recorded")
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, message)))
}

/// POST /api/v1/swipe/{book_id}/toggle
#[tracing::instrument(skip(pool, auth))]
pub async fn toggle_swipe(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    let result = SwipeService::toggle_swipe(pool.get_ref(), auth.user_id, book_id).await?;

    match result {
        Some(_swipe) => {
            Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "swipe.added"))))
        }
        None => {
            Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "swipe.removed"))))
        }
    }
}

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::config::SharedConfig;
use crate::dto::{ApiResponse, PaginationParams};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::ChatService;

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
pub struct StartChatRequest {
    pub user_id: i64,
    pub book_id: Option<i64>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SendMessageRequest {
    #[validate(length(min = 1, max = 5000, message = "validation.message_length"))]
    pub content: String,
    pub idempotency_key: Option<String>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ChatStartResponse {
    pub trade_id: i64,
    pub created: bool,
}

#[derive(Debug, Serialize)]
pub struct UnreadCountResponse {
    pub count: i64,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/chat/start
#[tracing::instrument(skip(pool, auth, body))]
pub async fn start_chat(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<StartChatRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let (trade_id, created) = ChatService::start_chat(
        pool.get_ref(),
        auth.user_id,
        body.user_id,
        body.book_id,
    ).await?;

    let data = ChatStartResponse { trade_id, created };

    if created {
        Ok(HttpResponse::Created().json(ApiResponse::success(data, i18n::t(&lang, "chat.started"))))
    } else {
        Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "chat.existing_conversation"))))
    }
}

/// GET /api/v1/trades/{id}/messages
#[tracing::instrument(skip(pool, auth, query))]
pub async fn get_messages(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let _lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let result = ChatService::get_messages(pool.get_ref(), trade_id, auth.user_id, &query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// POST /api/v1/trades/{id}/messages
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn send_message(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<SendMessageRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let message = ChatService::send_message(
        pool.get_ref(),
        trade_id,
        auth.user_id,
        &body.content,
        body.idempotency_key.as_deref(),
    ).await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(message, i18n::t(&lang, "chat.message_sent"))))
}

/// POST /api/v1/trades/{id}/messages/read
#[tracing::instrument(skip(pool, auth))]
pub async fn mark_read(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    ChatService::mark_messages_read(pool.get_ref(), trade_id, auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "chat.messages_marked_read"))))
}

/// GET /api/v1/inbox
#[tracing::instrument(skip(pool, auth, query))]
pub async fn get_inbox(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let _lang = lang_from_req(&req);

    let result = ChatService::get_inbox_paginated(pool.get_ref(), auth.user_id, &query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// GET /api/v1/inbox/unread-count
#[tracing::instrument(skip(pool, auth))]
pub async fn get_unread_count(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let count = ChatService::get_unread_count(pool.get_ref(), auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(UnreadCountResponse { count }, i18n::t(&lang, "general.success"))))
}

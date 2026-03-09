use actix_web::{web, HttpRequest, HttpResponse};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::config::SharedConfig;
use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::{Book, Trade};
use crate::services::TradeService;
use crate::services::trade::{CreateOfferData, CounterOfferData, FinalizeTradeData};
use crate::services::QueueService;

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
pub struct CreateOfferRequest {
    pub recipient_id: i64,
    pub offered_book_ids: Vec<i64>,
    pub requested_book_ids: Vec<i64>,
    #[serde(rename = "type")]
    #[validate(custom(function = "validate_offer_type"))]
    pub offer_type: String,
    pub cash_top_up: Option<Decimal>,
    pub delivery_method: Option<String>,
}

fn validate_offer_type(offer_type: &str) -> Result<(), validator::ValidationError> {
    match offer_type {
        "exchange" | "purchase" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid_offer_type")),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct AcceptOfferRequest {
    pub delivery_method: Option<String>,
    pub locker: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FinalizeRequest {
    pub payment_method_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CostPreviewQuery {
    pub delivery_method: Option<String>,
    pub locker: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CounterOfferRequest {
    pub offered_book_ids: Vec<i64>,
    pub requested_book_ids: Vec<i64>,
    pub cash_top_up: Option<Decimal>,
    pub delivery_method: Option<String>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OfferResponse {
    pub trade: Trade,
    pub offered_books: Vec<Book>,
    pub requested_books: Vec<Book>,
}

#[derive(Debug, Serialize)]
pub struct InventoryResponse {
    pub books: Vec<Book>,
}

#[derive(Debug, Serialize)]
pub struct CostPreviewResponse {
    pub shipping_cost: Decimal,
    pub protection_fee: Decimal,
    pub cash_top_up: Decimal,
    pub total: Decimal,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/offers/my-inventory
#[tracing::instrument(skip(pool, auth))]
pub async fn get_inventory(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let books = TradeService::get_user_inventory(pool.get_ref(), auth.user_id).await?;

    let data = InventoryResponse { books };
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/offers/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let detail = TradeService::get_offer(pool.get_ref(), trade_id, auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(detail, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/offers
#[tracing::instrument(skip(pool, auth, body))]
pub async fn create_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<CreateOfferRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let data = CreateOfferData {
        recipient_id: body.recipient_id,
        initiator_book_ids: body.offered_book_ids.clone(),
        recipient_book_ids: body.requested_book_ids.clone(),
        cash_top_up: body.cash_top_up,
        top_up_payer: None,
    };

    let detail = TradeService::create_offer(pool.get_ref(), auth.user_id, data).await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(detail, i18n::t(&lang, "offers.created"))))
}

/// POST /api/v1/offers/{id}/accept
#[tracing::instrument(skip(pool, auth, body))]
pub async fn accept_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<AcceptOfferRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let detail = TradeService::accept_offer(pool.get_ref(), trade_id, auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(detail, i18n::t(&lang, "offers.accepted"))))
}

/// POST /api/v1/offers/{id}/finalize
#[tracing::instrument(skip(pool, config, queue, auth, body))]
pub async fn finalize_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    queue: web::Data<QueueService>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<FinalizeRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let data = FinalizeTradeData {
        initiator_delivery_method: None,
        initiator_locker: None,
        recipient_delivery_method: None,
        recipient_locker: None,
    };

    let detail = TradeService::finalize_trade(pool.get_ref(), trade_id, auth.user_id, data).await?;

    // Dispatch shipment processing jobs via RabbitMQ instead of processing synchronously.
    // The trade details (lockers, provider, participants) come from the finalized trade record.
    if let (Some(initiator_locker), Some(recipient_locker)) = (
        detail.trade.initiator_locker.as_deref(),
        detail.trade.recipient_locker.as_deref(),
    ) {
        let provider = detail
            .trade
            .initiator_delivery_method
            .as_deref()
            .unwrap_or("inpost");

        // Initiator ships to recipient
        let _ = queue
            .dispatch_process_shipment(
                trade_id,
                "initiator_to_recipient",
                detail.trade.initiator_id,
                detail.trade.recipient_id,
                provider,
                initiator_locker,
                recipient_locker,
            )
            .await;

        // Recipient ships to initiator
        let _ = queue
            .dispatch_process_shipment(
                trade_id,
                "recipient_to_initiator",
                detail.trade.recipient_id,
                detail.trade.initiator_id,
                provider,
                recipient_locker,
                initiator_locker,
            )
            .await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(detail, i18n::t(&lang, "offers.finalized"))))
}

/// GET /api/v1/offers/{id}/cost-preview
#[tracing::instrument(skip(pool, auth, query))]
pub async fn cost_preview(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    query: web::Query<CostPreviewQuery>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let delivery_method = query.delivery_method.as_deref().unwrap_or("inpost");
    let preview = TradeService::get_cost_preview(pool.get_ref(), trade_id, delivery_method).await?;

    let data = CostPreviewResponse {
        shipping_cost: preview.initiator_shipping_cost,
        protection_fee: preview.protection_fee,
        cash_top_up: Decimal::ZERO,
        total: preview.total_initiator,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/offers/{id}/reject
#[tracing::instrument(skip(pool, auth))]
pub async fn reject_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let _detail = TradeService::reject_offer(pool.get_ref(), trade_id, auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "offers.rejected"))))
}

/// POST /api/v1/offers/{id}/cancel
#[tracing::instrument(skip(pool, auth))]
pub async fn cancel_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let _detail = TradeService::cancel_offer(pool.get_ref(), trade_id, auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "offers.cancelled"))))
}

/// POST /api/v1/offers/{id}/counter
#[tracing::instrument(skip(pool, auth, body))]
pub async fn counter_offer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<CounterOfferRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let data = CounterOfferData {
        initiator_book_ids: body.offered_book_ids.clone(),
        recipient_book_ids: body.requested_book_ids.clone(),
        cash_top_up: body.cash_top_up,
        top_up_payer: None,
    };

    let detail = TradeService::counter_offer(pool.get_ref(), trade_id, auth.user_id, data).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(detail, i18n::t(&lang, "offers.countered"))))
}

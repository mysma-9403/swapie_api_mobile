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
// Models used indirectly through services
use crate::services::{DeliveryService, TradeService};

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
pub struct DisputeRequest {
    #[validate(length(min = 10, max = 2000, message = "validation.dispute_reason_length"))]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct LockerSearchQuery {
    pub query: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ValidateLockerRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub locker_name: String,
}

#[derive(Debug, Deserialize)]
pub struct NearestLockerQuery {
    pub latitude: Decimal,
    pub longitude: Decimal,
    pub limit: Option<u32>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DeliveryOptionsResponse {
    pub options: Vec<crate::services::delivery::DeliveryOption>,
}

#[derive(Debug, Serialize)]
pub struct FeesResponse {
    pub protection_fee: Decimal,
    pub service_fee_percent: Decimal,
    pub delivery_options: Vec<crate::services::delivery::DeliveryOption>,
}

#[derive(Debug, Serialize)]
pub struct DeliveryStatusResponse {
    pub trade_id: i64,
    pub status: String,
    pub initiator_confirmed: bool,
    pub recipient_confirmed: bool,
    pub initiator_shipment_id: Option<String>,
    pub recipient_shipment_id: Option<String>,
    pub initiator_label_url: Option<String>,
    pub recipient_label_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LockerValidationResponse {
    pub valid: bool,
    pub locker: Option<LockerInfo>,
}

#[derive(Debug, Serialize)]
pub struct LockerInfo {
    pub name: String,
    pub address: String,
    pub city: String,
    pub latitude: Decimal,
    pub longitude: Decimal,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/config/delivery-options
#[tracing::instrument(skip(pool, auth))]
pub async fn get_delivery_options(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let options = DeliveryService::get_delivery_options(pool.get_ref()).await?;

    let data = DeliveryOptionsResponse { options };
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/config/fees
#[tracing::instrument(skip(pool, auth))]
pub async fn get_fees(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let fee_config = DeliveryService::get_fees(pool.get_ref()).await?;
    let delivery_options = DeliveryService::get_delivery_options(pool.get_ref()).await?;

    let data = FeesResponse {
        protection_fee: fee_config.protection_fee,
        service_fee_percent: fee_config.platform_fee_percent,
        delivery_options,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/trades/{id}/delivery-status
#[tracing::instrument(skip(pool, auth))]
pub async fn get_delivery_status(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    let status = TradeService::get_delivery_status(pool.get_ref(), trade_id).await?;

    let data = DeliveryStatusResponse {
        trade_id: status.trade_id,
        status: status.status,
        initiator_confirmed: status.initiator_confirmed_delivery,
        recipient_confirmed: status.recipient_confirmed_delivery,
        initiator_shipment_id: status.initiator_to_recipient_shipment_id,
        recipient_shipment_id: status.recipient_to_initiator_shipment_id,
        initiator_label_url: status.initiator_to_recipient_label_url,
        recipient_label_url: status.recipient_to_initiator_label_url,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/trades/{id}/confirm-delivery
#[tracing::instrument(skip(pool, auth))]
pub async fn confirm_delivery(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    TradeService::confirm_delivery(pool.get_ref(), trade_id, auth.user_id).await?;
    TradeService::check_trade_completion(pool.get_ref(), trade_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "delivery.confirmed"))))
}

/// POST /api/v1/trades/{id}/dispute
#[tracing::instrument(skip(pool, auth, body))]
pub async fn open_dispute(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<DisputeRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let trade_id = path.into_inner();

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    TradeService::open_dispute(pool.get_ref(), trade_id, auth.user_id, &body.reason).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "delivery.dispute_opened"))))
}

/// GET /api/v1/inpost/lockers/search
#[tracing::instrument(skip(pool, config, auth, query))]
pub async fn search_inpost(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    query: web::Query<LockerSearchQuery>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let search_query = query.query.as_deref().unwrap_or("");
    let lockers = DeliveryService::search_inpost_lockers(
        config.get_ref(),
        search_query,
        query.city.as_deref(),
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(lockers, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/inpost/lockers/validate
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn validate_inpost(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<ValidateLockerRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    match DeliveryService::validate_inpost_locker(config.get_ref(), &body.locker_name).await {
        Ok(locker) => {
            let data = LockerValidationResponse {
                valid: true,
                locker: Some(LockerInfo {
                    name: locker.name,
                    address: locker.address,
                    city: locker.city.unwrap_or_default(),
                    latitude: Decimal::from_f64_retain(locker.latitude).unwrap_or(Decimal::ZERO),
                    longitude: Decimal::from_f64_retain(locker.longitude).unwrap_or(Decimal::ZERO),
                }),
            };
            Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "delivery.locker_valid"))))
        }
        Err(_) => {
            let data = LockerValidationResponse {
                valid: false,
                locker: None,
            };
            Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "delivery.locker_not_found"))))
        }
    }
}

/// GET /api/v1/inpost/lockers/nearest
#[tracing::instrument(skip(pool, config, auth, query))]
pub async fn nearest_inpost(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    query: web::Query<NearestLockerQuery>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let limit = query.limit.unwrap_or(10).min(50) as i32;

    let lat = query.latitude.to_string().parse::<f64>().unwrap_or(0.0);
    let lng = query.longitude.to_string().parse::<f64>().unwrap_or(0.0);

    let lockers = DeliveryService::get_nearest_inpost(config.get_ref(), lat, lng, limit).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(lockers, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/inpost/lockers/{lockerName}
#[tracing::instrument(skip(pool, config, auth))]
pub async fn get_inpost_locker(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let locker_name = path.into_inner();

    let locker = DeliveryService::get_inpost_locker(config.get_ref(), &locker_name).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(locker, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/orlen/lockers/search
#[tracing::instrument(skip(pool, config, auth, query))]
pub async fn search_orlen(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    query: web::Query<LockerSearchQuery>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let search_query = query.query.as_deref().unwrap_or("");
    let lockers = DeliveryService::search_orlen_lockers(config.get_ref(), search_query).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(lockers, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/orlen/lockers/nearest
#[tracing::instrument(skip(pool, config, auth, query))]
pub async fn nearest_orlen(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    query: web::Query<NearestLockerQuery>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let limit = query.limit.unwrap_or(10).min(50) as i32;

    let lat = query.latitude.to_string().parse::<f64>().unwrap_or(0.0);
    let lng = query.longitude.to_string().parse::<f64>().unwrap_or(0.0);

    let lockers = DeliveryService::get_nearest_orlen(config.get_ref(), lat, lng, limit).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(lockers, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/orlen/lockers/{lockerName}
#[tracing::instrument(skip(pool, config, auth))]
pub async fn get_orlen_locker(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let locker_name = path.into_inner();

    let locker = DeliveryService::get_orlen_locker(config.get_ref(), &locker_name).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(locker, i18n::t(&lang, "general.success"))))
}

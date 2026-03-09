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
use crate::models::StripePaymentMethod;
use crate::services::StripeService;

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
pub struct WithdrawRequest {
    pub amount: Decimal,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TopupRequest {
    pub amount: Decimal,
    #[validate(length(min = 1, message = "validation.required"))]
    pub payment_method_id: String,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub balance: Decimal,
    pub pending: Decimal,
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct StripeConfigResponse {
    pub publishable_key: String,
}

#[derive(Debug, Serialize)]
pub struct CustomerResponse {
    pub stripe_customer_id: String,
}

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub has_customer: bool,
    pub has_payment_method: bool,
    pub has_connect_account: bool,
    pub connect_charges_enabled: bool,
    pub connect_payouts_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct SetupIntentResponse {
    pub client_secret: String,
}

#[derive(Debug, Serialize)]
pub struct TopupResponse {
    pub payment_intent_id: String,
    pub client_secret: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct PaymentStatusResponse {
    pub status: String,
    pub amount: Option<Decimal>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/wallet/balance
#[tracing::instrument(skip(pool, auth))]
pub async fn get_balance(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let wallet = StripeService::get_wallet_balance(pool.get_ref(), auth.user_id).await?;

    let data = BalanceResponse {
        balance: wallet.balance,
        pending: Decimal::ZERO,
        currency: wallet.currency,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/wallet/withdraw
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn withdraw(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<WithdrawRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    if body.amount < Decimal::new(1, 0) {
        return Err(ApiError::validation("validation.min_amount"));
    }

    StripeService::request_withdrawal(pool.get_ref(), auth.user_id, body.amount).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.withdrawal_initiated"))))
}

/// GET /api/v1/stripe/config
#[tracing::instrument(skip(config, auth))]
pub async fn get_stripe_config(
    req: HttpRequest,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let stripe_config = StripeService::get_config(config.get_ref());

    let data = StripeConfigResponse {
        publishable_key: stripe_config.publishable_key,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/stripe/customer
#[tracing::instrument(skip(pool, config, auth))]
pub async fn ensure_customer(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let customer_id = StripeService::ensure_customer(config.get_ref(), pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(
        CustomerResponse {
            stripe_customer_id: customer_id,
        },
        i18n::t(&lang, "payments.customer_created"),
    )))
}

/// GET /api/v1/stripe/readiness
#[tracing::instrument(skip(pool, auth))]
pub async fn get_readiness(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let readiness = StripeService::get_readiness(pool.get_ref(), auth.user_id).await?;

    let data = ReadinessResponse {
        has_customer: readiness.has_stripe_customer,
        has_payment_method: readiness.has_payment_method,
        has_connect_account: readiness.has_connect_account,
        connect_charges_enabled: readiness.connect_charges_enabled,
        connect_payouts_enabled: readiness.connect_payouts_enabled,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/stripe/setup-intent
#[tracing::instrument(skip(pool, config, auth))]
pub async fn create_setup_intent(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let customer_id = StripeService::ensure_customer(config.get_ref(), pool.get_ref(), auth.user_id).await?;
    let intent = StripeService::create_setup_intent(config.get_ref(), &customer_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(
        SetupIntentResponse {
            client_secret: intent.client_secret,
        },
        i18n::t(&lang, "general.success"),
    )))
}

/// GET /api/v1/stripe/payment-methods
#[tracing::instrument(skip(pool, auth))]
pub async fn list_payment_methods(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let methods = StripeService::list_payment_methods(pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(methods, i18n::t(&lang, "general.success"))))
}

/// DELETE /api/v1/stripe/payment-methods/{id}
#[tracing::instrument(skip(pool, config, auth))]
pub async fn delete_payment_method(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let method_id = path.into_inner();

    // Look up the stripe_payment_method_id from the local record
    let method = sqlx::query_as::<_, StripePaymentMethod>(
        "SELECT * FROM stripe_payment_methods WHERE id = ? AND user_id = ?",
    )
    .bind(method_id)
    .bind(auth.user_id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("payments.method_not_found"))?;

    StripeService::delete_payment_method(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
        &method.stripe_payment_method_id,
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.method_deleted"))))
}

/// PUT /api/v1/stripe/payment-methods/{id}/default
#[tracing::instrument(skip(pool, auth))]
pub async fn set_default_payment_method(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let method_id = path.into_inner();

    // Look up the stripe_payment_method_id from the local record
    let method = sqlx::query_as::<_, StripePaymentMethod>(
        "SELECT * FROM stripe_payment_methods WHERE id = ? AND user_id = ?",
    )
    .bind(method_id)
    .bind(auth.user_id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("payments.method_not_found"))?;

    StripeService::set_default_payment_method(
        pool.get_ref(),
        auth.user_id,
        &method.stripe_payment_method_id,
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.default_set"))))
}

/// POST /api/v1/stripe/topup
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn create_topup(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<TopupRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    if body.amount < Decimal::new(1, 0) {
        return Err(ApiError::validation("validation.min_amount"));
    }

    let intent = StripeService::create_topup_intent(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
        body.amount,
    )
    .await?;

    let data = TopupResponse {
        payment_intent_id: intent.payment_intent_id,
        client_secret: Some(intent.client_secret),
        status: "pending".to_string(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "payments.topup_created"))))
}

/// GET /api/v1/stripe/payment/{paymentIntentId}/status
#[tracing::instrument(skip(pool, auth))]
pub async fn get_payment_status(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let payment_intent_id = path.into_inner();

    let payment = StripeService::get_payment_status(pool.get_ref(), &payment_intent_id).await?;

    let data = PaymentStatusResponse {
        status: payment.status.to_string(),
        amount: Some(payment.amount),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/stripe/webhook [NO AUTH - validates signature]
#[tracing::instrument(skip(pool, config, body, req))]
pub async fn handle_webhook(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let signature = req
        .headers()
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("payments.missing_signature"))?;

    let payload = String::from_utf8(body.to_vec())
        .map_err(|_| ApiError::bad_request("payments.invalid_payload"))?;

    StripeService::handle_webhook(config.get_ref(), pool.get_ref(), &payload, signature).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"received": true})))
}

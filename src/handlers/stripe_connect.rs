use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::config::SharedConfig;
use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::StripeConnectService;
use crate::services::stripe_connect::{
    CreateConnectData, UpdateConnectData, AddBankAccountData,
};

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
pub struct CreateConnectAccountRequest {
    pub country: Option<String>,
    pub business_type: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateConnectAccountRequest {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub dob_day: Option<u8>,
    pub dob_month: Option<u8>,
    pub dob_year: Option<u16>,
    pub address_line1: Option<String>,
    pub address_city: Option<String>,
    pub address_postal_code: Option<String>,
    pub address_country: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddBankAccountRequest {
    #[validate(length(min = 1, message = "validation.required"))]
    pub account_number: String,
    pub routing_number: Option<String>,
    pub country: Option<String>,
    pub currency: Option<String>,
    pub account_holder_name: Option<String>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ConnectAccountResponse {
    pub account_id: String,
    pub status: String,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct ConnectStatusResponse {
    pub account_id: Option<String>,
    pub status: Option<String>,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub onboarded_at: Option<chrono::NaiveDateTime>,
    pub requirements: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct BankAccountInfo {
    pub id: String,
    pub last4: String,
    pub bank_name: Option<String>,
    pub country: String,
    pub currency: String,
    pub default_for_currency: bool,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/stripe/connect/account
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn create_account(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<CreateConnectAccountRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let data = CreateConnectData {
        business_type: body.business_type.clone(),
        country: body.country.clone(),
        email: None,
        first_name: None,
        last_name: None,
    };

    let account_id = StripeConnectService::create_connect_account(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
        data,
    )
    .await?;

    let response = ConnectAccountResponse {
        account_id,
        status: "pending".to_string(),
        charges_enabled: false,
        payouts_enabled: false,
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(response, i18n::t(&lang, "payments.connect_account_created"))))
}

/// PUT /api/v1/stripe/connect/account
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn update_account(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateConnectAccountRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let data = UpdateConnectData {
        business_type: None,
        email: body.email.clone(),
        first_name: body.first_name.clone(),
        last_name: body.last_name.clone(),
        phone: body.phone.clone(),
        dob_day: body.dob_day.map(|v| v as i32),
        dob_month: body.dob_month.map(|v| v as i32),
        dob_year: body.dob_year.map(|v| v as i32),
        address_line1: body.address_line1.clone(),
        address_city: body.address_city.clone(),
        address_postal_code: body.address_postal_code.clone(),
        address_country: body.address_country.clone(),
    };

    StripeConnectService::update_connect_account(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
        data,
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.connect_account_updated"))))
}

/// POST /api/v1/stripe/connect/documents
#[tracing::instrument(skip(pool, config, auth, payload))]
pub async fn submit_documents(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    mut payload: Multipart,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|_| ApiError::bad_request("general.invalid_multipart"))?;
        let filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|f| f.to_string()))
            .unwrap_or_else(|| "document.pdf".to_string());

        let mut bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|_| ApiError::bad_request("general.upload_error"))?;
            bytes.extend_from_slice(&data);
        }
        files.push((filename, bytes));
    }

    // Upload each file to Stripe Files API to get file IDs
    let mut file_ids: Vec<String> = Vec::new();
    for (filename, file_data) in &files {
        let client = reqwest::Client::new();
        let form = reqwest::multipart::Form::new()
            .text("purpose", "identity_document")
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_data.clone())
                    .file_name(filename.clone()),
            );
        let file_response = client
            .post("https://files.stripe.com/v1/files")
            .header(
                "Authorization",
                format!("Bearer {}", config.stripe_secret_key),
            )
            .multipart(form)
            .send()
            .await
            .map_err(|_| ApiError::external_service("payment.stripe_error"))?;

        let file_json: serde_json::Value = file_response
            .json()
            .await
            .map_err(|_| ApiError::external_service("payment.stripe_error"))?;

        if let Some(file_id) = file_json.get("id").and_then(|v| v.as_str()) {
            file_ids.push(file_id.to_string());
        }
    }

    // Attach document file IDs to the Connect account via service
    if let Some(front_id) = file_ids.first() {
        let back_id = file_ids.get(1).map(|s| s.as_str());
        StripeConnectService::submit_documents(
            config.get_ref(),
            pool.get_ref(),
            auth.user_id,
            front_id,
            back_id,
        )
        .await?;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.documents_submitted"))))
}

/// GET /api/v1/stripe/connect/status
#[tracing::instrument(skip(pool, config, auth))]
pub async fn get_status(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let status = StripeConnectService::get_connect_status(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
    )
    .await?;

    let requirements_value = if status.requirements.is_empty() {
        None
    } else {
        Some(serde_json::to_value(&status.requirements).unwrap_or_default())
    };

    let data = ConnectStatusResponse {
        account_id: status.account_id,
        status: status.status,
        charges_enabled: status.charges_enabled,
        payouts_enabled: status.payouts_enabled,
        onboarded_at: status.onboarded_at,
        requirements: requirements_value,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/stripe/connect/accept-tos
#[tracing::instrument(skip(pool, config, auth))]
pub async fn accept_tos(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    StripeConnectService::accept_tos(config.get_ref(), pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.tos_accepted"))))
}

/// POST /api/v1/stripe/connect/bank-account
#[tracing::instrument(skip(pool, config, auth, body))]
pub async fn add_bank_account(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    body: web::Json<AddBankAccountRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let data = AddBankAccountData {
        account_holder_name: body.account_holder_name.clone().unwrap_or_default(),
        account_holder_type: None,
        routing_number: body.routing_number.clone(),
        account_number: body.account_number.clone(),
        country: body.country.clone().unwrap_or_else(|| "PL".to_string()),
        currency: body.currency.clone().unwrap_or_else(|| "pln".to_string()),
    };

    let bank_account = StripeConnectService::add_bank_account(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
        data,
    )
    .await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(bank_account, i18n::t(&lang, "payments.bank_account_added"))))
}

/// GET /api/v1/stripe/connect/bank-accounts
#[tracing::instrument(skip(pool, config, auth))]
pub async fn list_bank_accounts(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let accounts = StripeConnectService::list_bank_accounts(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(accounts, i18n::t(&lang, "general.success"))))
}

/// DELETE /api/v1/stripe/connect/bank-accounts/{bankAccountId}
#[tracing::instrument(skip(pool, config, auth))]
pub async fn remove_bank_account(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let bank_account_id = path.into_inner();

    StripeConnectService::remove_bank_account(
        config.get_ref(),
        pool.get_ref(),
        auth.user_id,
        &bank_account_id,
    )
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "payments.bank_account_removed"))))
}

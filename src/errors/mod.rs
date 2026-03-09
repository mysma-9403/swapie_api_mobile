use actix_web::{HttpResponse, http::StatusCode};
use serde::Serialize;
use thiserror::Error;

use crate::i18n;

#[derive(Debug, Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
    errors: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Payment error: {0}")]
    PaymentError(String),

    #[error("External service error: {0}")]
    ExternalServiceError(String),
}

impl ApiError {
    /// Returns the i18n message key carried by this error variant.
    pub fn message_key(&self) -> &str {
        match self {
            ApiError::BadRequest(key) => key,
            ApiError::Unauthorized(key) => key,
            ApiError::Forbidden(key) => key,
            ApiError::NotFound(key) => key,
            ApiError::Conflict(key) => key,
            ApiError::ValidationError(key) => key,
            ApiError::InternalError(key) => key,
            ApiError::RateLimited(key) => key,
            ApiError::PaymentError(key) => key,
            ApiError::ExternalServiceError(key) => key,
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            ApiError::PaymentError(_) => StatusCode::PAYMENT_REQUIRED,
            ApiError::ExternalServiceError(_) => StatusCode::BAD_GATEWAY,
        }
    }

    // ── Helper constructors ──────────────────────────────────────────

    pub fn bad_request(key: impl Into<String>) -> Self {
        ApiError::BadRequest(key.into())
    }

    pub fn unauthorized(key: impl Into<String>) -> Self {
        ApiError::Unauthorized(key.into())
    }

    pub fn forbidden() -> Self {
        ApiError::Forbidden("general.forbidden".into())
    }

    pub fn not_found(key: impl Into<String>) -> Self {
        ApiError::NotFound(key.into())
    }

    pub fn conflict(key: impl Into<String>) -> Self {
        ApiError::Conflict(key.into())
    }

    pub fn validation(key: impl Into<String>) -> Self {
        ApiError::ValidationError(key.into())
    }

    pub fn internal() -> Self {
        ApiError::InternalError("general.server_error".into())
    }

    pub fn rate_limited() -> Self {
        ApiError::RateLimited("general.rate_limited".into())
    }

    pub fn payment(key: impl Into<String>) -> Self {
        ApiError::PaymentError(key.into())
    }

    pub fn external_service(key: impl Into<String>) -> Self {
        ApiError::ExternalServiceError(key.into())
    }

    /// Build the JSON response for a given language.
    pub fn to_response(&self, lang: &str) -> HttpResponse {
        let message = i18n::t(lang, self.message_key());
        let body = ErrorResponse {
            success: false,
            message,
            errors: None,
        };
        HttpResponse::build(self.status_code()).json(body)
    }

    /// Build the JSON response with additional field-level errors.
    pub fn to_response_with_errors(&self, lang: &str, errors: Vec<String>) -> HttpResponse {
        let message = i18n::t(lang, self.message_key());
        let body = ErrorResponse {
            success: false,
            message,
            errors: Some(errors),
        };
        HttpResponse::build(self.status_code()).json(body)
    }
}

impl actix_web::ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.status_code()
    }

    fn error_response(&self) -> HttpResponse {
        // Default to English when no request context is available.
        self.to_response("en")
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!("Database error: {:?}", err);
        ApiError::internal()
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        tracing::error!("External service error: {:?}", err);
        ApiError::external_service("general.server_error")
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        tracing::error!("Serialization error: {:?}", err);
        ApiError::bad_request("general.server_error")
    }
}

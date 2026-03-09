use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::Quota;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::errors::ApiError;

// ── Rate Limit Configuration ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_second: u32,
    pub burst_size: u32,
}

impl RateLimitConfig {
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        Self {
            requests_per_second,
            burst_size,
        }
    }

    /// Default rate limit: 60 requests/minute (1 per second, burst of 60).
    pub fn default_limit() -> Self {
        Self::new(1, 60)
    }

    /// Auth rate limit: 5 requests/minute.
    pub fn auth_limit() -> Self {
        Self::new(1, 5)
    }

    /// Registration rate limit: 10 requests/minute.
    pub fn registration_limit() -> Self {
        Self::new(1, 10)
    }

    /// Payment rate limit: 10 requests/minute.
    pub fn payment_limit() -> Self {
        Self::new(1, 10)
    }

    /// Webhook rate limit: 100 requests/minute.
    pub fn webhook_limit() -> Self {
        Self::new(2, 100)
    }
}

// ── Rate Limiter (stored as app data) ───────────────────────────────────

type GovernorLimiter = governor::RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// A simple rate limiter that can be stored in `actix_web::web::Data` and
/// checked explicitly in handlers.
///
/// # Example
/// ```ignore
/// async fn my_handler(limiter: web::Data<RateLimiter>) -> Result<HttpResponse, ApiError> {
///     limiter.check()?;
///     // ... handle request ...
/// }
/// ```
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<GovernorLimiter>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(config.requests_per_second).expect("requests_per_second must be > 0"),
        )
        .allow_burst(
            NonZeroU32::new(config.burst_size).expect("burst_size must be > 0"),
        );

        let limiter = Arc::new(governor::RateLimiter::direct(quota));
        Self { inner: limiter }
    }

    /// Create with the default limit (60 req/min).
    pub fn default_limit() -> Self {
        Self::new(RateLimitConfig::default_limit())
    }

    /// Create with the auth limit (5 req/min).
    pub fn auth_limit() -> Self {
        Self::new(RateLimitConfig::auth_limit())
    }

    /// Create with the registration limit (10 req/min).
    pub fn registration_limit() -> Self {
        Self::new(RateLimitConfig::registration_limit())
    }

    /// Create with the payment limit (10 req/min).
    pub fn payment_limit() -> Self {
        Self::new(RateLimitConfig::payment_limit())
    }

    /// Create with the webhook limit (100 req/min).
    pub fn webhook_limit() -> Self {
        Self::new(RateLimitConfig::webhook_limit())
    }

    /// Check whether the request is within the rate limit.
    /// Returns `Ok(())` if allowed, or `Err(ApiError::RateLimited)` if exceeded.
    pub fn check(&self) -> Result<(), ApiError> {
        self.inner.check().map_err(|_| {
            ApiError::RateLimited("Too many requests. Please try again later.".to_string())
        })
    }
}

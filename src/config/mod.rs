mod database;
mod app;
pub mod rabbitmq;

pub use database::DatabaseConfig;
pub use app::AppConfig;
pub use rabbitmq::RabbitMQConfig;

use std::sync::Arc;

/// Central configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub rabbitmq: RabbitMQConfig,
    pub jwt_secret: String,
    pub jwt_expiration_days: i64,
    pub stripe_secret_key: String,
    pub stripe_publishable_key: String,
    pub stripe_webhook_secret: String,
    pub stripe_connect_webhook_secret: String,
    pub inpost_api_base_url: String,
    pub inpost_api_token: String,
    pub orlen_api_base_url: String,
    pub orlen_api_token: String,
    pub fcm_server_key: String,
    pub fcm_project_id: String,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_url: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from_email: String,
    pub smtp_from_name: String,
    pub redis_url: String,
    pub sms_api_key: String,
    pub sms_api_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub apple_client_id: String,
    pub apple_team_id: String,
    pub apple_key_id: String,
    pub apple_private_key: String,
    pub recaptcha_secret: String,
    pub book_api_url: String,
    pub demo_mode: bool,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            app: AppConfig::from_env(),
            database: DatabaseConfig::from_env(),
            rabbitmq: RabbitMQConfig::from_env(),
            jwt_secret: env_or("JWT_SECRET", "change-me-in-production"),
            jwt_expiration_days: env_or("JWT_EXPIRATION_DAYS", "30")
                .parse()
                .unwrap_or(30),
            stripe_secret_key: env_or("STRIPE_SECRET_KEY", ""),
            stripe_publishable_key: env_or("STRIPE_PUBLISHABLE_KEY", ""),
            stripe_webhook_secret: env_or("STRIPE_WEBHOOK_SECRET", ""),
            stripe_connect_webhook_secret: env_or("STRIPE_CONNECT_WEBHOOK_SECRET", ""),
            inpost_api_base_url: env_or(
                "INPOST_API_BASE_URL",
                "https://api.inpost-group.com/points",
            ),
            inpost_api_token: env_or("INPOST_API_TOKEN", ""),
            orlen_api_base_url: env_or("ORLEN_API_BASE_URL", ""),
            orlen_api_token: env_or("ORLEN_API_TOKEN", ""),
            fcm_server_key: env_or("FCM_SERVER_KEY", ""),
            fcm_project_id: env_or("FCM_PROJECT_ID", ""),
            s3_endpoint: env_or("S3_ENDPOINT", ""),
            s3_bucket: env_or("S3_BUCKET", "swapie-media"),
            s3_region: env_or("S3_REGION", "ams3"),
            s3_access_key: env_or("S3_ACCESS_KEY", ""),
            s3_secret_key: env_or("S3_SECRET_KEY", ""),
            s3_url: env_or("S3_URL", ""),
            smtp_host: env_or("SMTP_HOST", "localhost"),
            smtp_port: env_or("SMTP_PORT", "587").parse().unwrap_or(587),
            smtp_username: env_or("SMTP_USERNAME", ""),
            smtp_password: env_or("SMTP_PASSWORD", ""),
            smtp_from_email: env_or("SMTP_FROM_EMAIL", "noreply@swapie.app"),
            smtp_from_name: env_or("SMTP_FROM_NAME", "Swapie"),
            redis_url: env_or("REDIS_URL", "redis://127.0.0.1:6379"),
            sms_api_key: env_or("SMS_API_KEY", ""),
            sms_api_url: env_or("SMS_API_URL", ""),
            google_client_id: env_or("GOOGLE_CLIENT_ID", ""),
            google_client_secret: env_or("GOOGLE_CLIENT_SECRET", ""),
            apple_client_id: env_or("APPLE_CLIENT_ID", ""),
            apple_team_id: env_or("APPLE_TEAM_ID", ""),
            apple_key_id: env_or("APPLE_KEY_ID", ""),
            apple_private_key: env_or("APPLE_PRIVATE_KEY", ""),
            recaptcha_secret: env_or("RECAPTCHA_SECRET", ""),
            book_api_url: env_or("BOOK_API_URL", "https://library.lagano.pl"),
            demo_mode: env_or("DEMO_MODE", "false")
                .parse()
                .unwrap_or(false),
        }
    }
}

pub type SharedConfig = Arc<Config>;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

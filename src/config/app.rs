use super::env_or;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub name: String,
    pub env: String,
    pub host: String,
    pub port: u16,
    pub url: String,
    pub debug: bool,
    pub workers: usize,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            name: env_or("APP_NAME", "Swapie"),
            env: env_or("APP_ENV", "production"),
            host: env_or("APP_HOST", "0.0.0.0"),
            port: env_or("APP_PORT", "8080").parse().unwrap_or(8080),
            url: env_or("APP_URL", "http://localhost:8080"),
            debug: env_or("APP_DEBUG", "false").parse().unwrap_or(false),
            workers: env_or("APP_WORKERS", "4").parse().unwrap_or(4),
        }
    }

    pub fn is_production(&self) -> bool {
        self.env == "production"
    }
}

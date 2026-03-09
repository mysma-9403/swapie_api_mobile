use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

use super::env_or;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

impl DatabaseConfig {
    pub fn from_env() -> Self {
        Self {
            url: env_or("DATABASE_URL", "mysql://root:@127.0.0.1:3306/swapie"),
            max_connections: env_or("DATABASE_MAX_CONNECTIONS", "20")
                .parse()
                .unwrap_or(20),
            min_connections: env_or("DATABASE_MIN_CONNECTIONS", "5")
                .parse()
                .unwrap_or(5),
        }
    }

    pub async fn create_pool(&self) -> Result<MySqlPool, sqlx::Error> {
        MySqlPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .idle_timeout(std::time::Duration::from_secs(300))
            .max_lifetime(std::time::Duration::from_secs(1800))
            .connect(&self.url)
            .await
    }
}

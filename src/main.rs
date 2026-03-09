#![allow(unused_imports, unused_variables, dead_code)]

use std::sync::Arc;

use actix_cors::Cors;
use actix_web::web::{Data, JsonConfig};
use actix_web::{middleware::Logger, App, HttpServer};
use sqlx::mysql::MySqlPool;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::EnvFilter;

mod config;
mod dto;
mod errors;
mod handlers;
mod i18n;
mod middleware;
mod models;
mod routes;
mod services;
mod utils;
mod workers;

use config::{Config, SharedConfig};
use services::QueueService;

/// Shared application state available to all handlers via `web::Data`.
pub struct AppState {
    pub pool: MySqlPool,
    pub config: SharedConfig,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // ── Load environment variables ──────────────────────────────────────
    dotenvy::dotenv().ok();

    // ── Initialize tracing / logging ────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();

    tracing::info!("Starting Swapie Backend...");

    // ── Load configuration ──────────────────────────────────────────────
    let config = Config::from_env();
    let shared_config: SharedConfig = Arc::new(config.clone());

    let bind_addr = format!("{}:{}", config.app.host, config.app.port);
    let workers = config.app.workers;

    tracing::info!(
        "Environment: {} | Debug: {} | Bind: {} | Workers: {}",
        config.app.env,
        config.app.debug,
        bind_addr,
        workers,
    );

    // ── Create database connection pool ─────────────────────────────────
    let pool = config
        .database
        .create_pool()
        .await
        .expect("Failed to create database pool");

    tracing::info!(
        "Database pool created (max_connections={})",
        config.database.max_connections,
    );

    // ── Run migrations (controlled by RUN_MIGRATIONS env var) ───────────
    if std::env::var("RUN_MIGRATIONS")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false)
    {
        tracing::info!("Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run database migrations");
        tracing::info!("Migrations completed successfully");
    }

    // ── Check for worker mode ───────────────────────────────────────────
    // Usage: cargo run -- --worker <queue_name>
    // Example: cargo run -- --worker emails
    //          cargo run -- --worker shipments
    //          cargo run -- --worker sms
    //          cargo run -- --worker notifications
    //          cargo run -- --worker default
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--worker") {
        let queue_name = args
            .get(pos + 1)
            .expect("Usage: cargo run -- --worker <queue_name>");
        tracing::info!("Starting in worker mode for queue: {}", queue_name);
        let processor =
            workers::processor::JobProcessor::new(shared_config.clone(), pool.clone());
        processor
            .run(queue_name)
            .await
            .expect("Worker exited with error");
        return Ok(());
    }

    // ── Connect to RabbitMQ and declare queues ──────────────────────────
    let queue_service = QueueService::new(shared_config.clone())
        .await
        .expect("Failed to connect to RabbitMQ");

    queue_service
        .declare_queues()
        .await
        .expect("Failed to declare RabbitMQ queues");

    // ── Start the scheduler (background periodic tasks) ─────────────────
    let scheduler = Arc::new(workers::scheduler::Scheduler::new(
        shared_config.clone(),
        pool.clone(),
        queue_service.clone(),
    ));
    scheduler.start();

    // ── Build and start the HTTP server ─────────────────────────────────
    let pool_data = Data::new(pool.clone());
    let config_data = Data::new(shared_config.clone());
    let queue_data = Data::new(queue_service);

    tracing::info!("Server starting at {}", bind_addr);

    HttpServer::new(move || {
        // CORS configuration — allow mobile app origins
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://localhost:8080")
            .allowed_origin("https://swapie.app")
            .allowed_origin("https://api.swapie.app")
            .allow_any_method()
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);

        // JSON payload configuration
        let json_cfg = JsonConfig::default()
            .limit(10 * 1024 * 1024) // 10 MB
            .error_handler(|err, _req| {
                let response = actix_web::HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "message": format!("Invalid JSON payload: {}", err),
                }));
                actix_web::error::InternalError::from_response(err, response).into()
            });

        App::new()
            // Shared state
            .app_data(pool_data.clone())
            .app_data(config_data.clone())
            .app_data(queue_data.clone())
            .app_data(json_cfg)
            // Middleware
            .wrap(cors)
            .wrap(TracingLogger::default())
            .wrap(Logger::new(
                "%a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
            // Routes
            .configure(routes::configure_routes)
    })
    .bind(&bind_addr)?
    .workers(workers)
    .run()
    .await
}

mod api;
mod config;
mod crypto;
mod db;
mod registry;
mod services;

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use clap::Parser;
use config::{CliArgs, Config};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Inicializace loggingu
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "simple_release_management=info,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Simple Release Management");

    // Parse CLI argumentů
    let cli = CliArgs::parse();

    // Načtení konfigurace (CLI argumenty mají prioritu)
    let config = Config::from_env_and_cli(cli).context("Failed to load configuration")?;
    info!("Configuration loaded");
    info!("Server will listen on: {}", config.server_address());
    info!("Base path: {}", if config.base_path.is_empty() { "/" } else { &config.base_path });

    // Připojení k databázi
    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .connect(&config.database_url)
        .await
        .context("Failed to connect to database")?;

    info!("Database connected successfully");

    // Spuštění migrací
    info!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("Failed to run database migrations")?;

    info!("Database migrations completed successfully");

    // Inicializace Skopeo service
    let skopeo_service = services::SkopeoService::new(config.skopeo_path.clone());

    // Zkontrolovat že skopeo je dostupné
    match skopeo_service.check_available().await {
        Ok(true) => info!("Skopeo is available"),
        Ok(false) => {
            tracing::warn!("Skopeo check returned false");
        }
        Err(e) => {
            tracing::warn!("Skopeo is not available: {}. Copy operations will fail.", e);
        }
    }

    // Vytvoření API routeru
    let api_router = api::create_api_router(pool.clone(), config.encryption_secret.clone());

    // Vytvoření copy API state
    let copy_state = api::copy::CopyApiState {
        pool: pool.clone(),
        skopeo: skopeo_service,
        encryption_secret: config.encryption_secret.clone(),
        job_logs: Arc::new(RwLock::new(std::collections::HashMap::new())),
    };

    // Vytvoření copy API routeru
    let copy_router = api::copy::router(copy_state);

    // Statické soubory
    let serve_dir = ServeDir::new("src/web/static")
        .not_found_service(ServeDir::new("src/web/static/index.html"));

    // Vytvoření kompletního routeru
    let app = Router::new()
        .route("/health", get(health_handler))
        .merge(api_router)
        .nest("/api/v1", copy_router)
        .fallback_service(serve_dir);

    info!("Application initialized successfully");
    info!("Starting HTTP server on {}", config.server_address());

    // Spuštění serveru
    let listener = tokio::net::TcpListener::bind(&config.server_address())
        .await
        .context("Failed to bind server address")?;

    info!("Server is ready to accept connections on {}", config.server_address());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server error")?;

    info!("Server shutdown complete");

    Ok(())
}

/// Health check handler
async fn health_handler() -> &'static str {
    "OK"
}

/// Graceful shutdown signal
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");

    info!("Shutdown signal received, cleaning up...");
}

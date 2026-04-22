mod api;
mod auth;
mod config;
mod crypto;
mod db;
mod registry;
mod services;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    middleware,
    response::Response,
    routing::get,
    Extension, Router,
};
use clap::Parser;
use config::{CliArgs, Config};
use rust_embed::RustEmbed;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(RustEmbed)]
#[folder = "src/web/static/"]
struct EmbeddedWebAssets;

async fn embedded_static_handler(uri: Uri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');
    let asset_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };

    if let Some(response) = embedded_asset_response(asset_path) {
        return response;
    }

    if let Some(response) = embedded_asset_response("index.html") {
        return response;
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Embedded asset not found"))
        .unwrap()
}

fn embedded_asset_response(path: &str) -> Option<Response> {
    let asset = EmbeddedWebAssets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let mut response = Response::new(Body::from(asset.data.into_owned()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    Some(response)
}

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
    info!("Image tool: {} ({})", config.image_tool, config.image_tool_path);
    info!(
        "Authorization: {}",
        if config.auth_enabled { "enabled" } else { "DISABLED (development mode)" }
    );

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

    // Inicializace image tool service
    let skopeo_service = services::ImageToolService::new(
        config.image_tool.clone(),
        config.image_tool_path.clone(),
        config.image_tool_src_insecure,
        config.image_tool_dst_insecure,
        config.image_tool_extra_inspect_args.clone(),
        config.image_tool_extra_copy_args.clone(),
    );

    // Zkontrolovat že image tool je dostupný
    match skopeo_service.check_available().await {
        Ok(true) => info!("Image tool is available"),
        Ok(false) => {
            tracing::warn!("Image tool check returned false");
        }
        Err(e) => {
            tracing::warn!("Image tool is not available: {}. Copy operations will fail.", e);
        }
    }

    // Vytvoření API routeru
    let api_router = api::create_api_router(
        pool.clone(),
        config.encryption_secret.clone(),
        config.image_tool.clone(),
        config.image_tool_path.clone(),
    );

    // Vytvoření copy API state
    let copy_state = api::copy::CopyApiState {
        pool: pool.clone(),
        skopeo: skopeo_service,
        encryption_secret: config.encryption_secret.clone(),
        job_logs: Arc::new(RwLock::new(std::collections::HashMap::new())),
        cancel_flags: Arc::new(RwLock::new(std::collections::HashSet::new())),
    };

    // Vytvoření copy API routeru
    let copy_router = api::copy::router(copy_state);

    // Vytvoření deploy API state
    let deploy_state = api::deploy::DeployApiState {
        pool: pool.clone(),
        encryption_secret: config.encryption_secret.clone(),
        kube_build_app_path: config.kube_build_app_path.clone(),
        apply_env_path: config.apply_env_path.clone(),
        encjson_path: config.encjson_path.clone(),
        encjson_legacy_path: config.encjson_legacy_path.clone(),
        encjson_key_dir: config.encjson_key_dir.clone(),
        kubeconform_path: config.kubeconform_path.clone(),
        job_logs: Arc::new(RwLock::new(std::collections::HashMap::new())),
    };

    let deploy_router = api::deploy::router(deploy_state);

    // Vytvoření kompletního routeru
    let mut app = Router::new()
        .route("/health", get(health_handler))
        .merge(api_router)
        .nest("/api/v1", copy_router)
        .nest("/api/v1", deploy_router)
        .layer(Extension(pool.clone()));

    if let Some(static_dir) = config.static_dir.clone() {
        info!("Static assets: filesystem ({})", static_dir);
        let static_index = std::path::Path::new(&static_dir).join("index.html");
        let serve_dir = ServeDir::new(&static_dir)
            .not_found_service(ServeDir::new(static_index));
        app = app.fallback_service(serve_dir);
    } else {
        info!("Static assets: embedded");
        app = app.fallback(get(embedded_static_handler));
    }

    let app = if config.auth_enabled {
        app.layer(middleware::from_fn(auth::auth_middleware))
    } else {
        app.layer(middleware::from_fn(auth::auth_disabled_middleware))
    };

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

    // In dev, force exit to avoid hanging on long-lived SSE connections.
    if cfg!(debug_assertions) {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        std::process::exit(0);
    }
}

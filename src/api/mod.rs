pub mod bundles;
pub mod copy;
pub mod deploy;
pub mod git_repos;
pub mod registries;
pub mod releases;
pub mod tenants;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;

/// Vytvoří router s všemi API endpointy
pub fn create_api_router(pool: PgPool, encryption_secret: String) -> Router {
    let registry_state = registries::RegistryApiState {
        pool: pool.clone(),
        encryption_secret,
    };

    let git_repo_state = git_repos::GitRepoApiState {
        pool: pool.clone(),
        encryption_secret: registry_state.encryption_secret.clone(),
    };

    let api_v1 = Router::new()
        .nest("/tenants", tenants::router(pool.clone()))
        .merge(registries::router(registry_state))
        .merge(git_repos::router(git_repo_state))
        .merge(bundles::router(pool.clone()))
        .merge(releases::router(pool.clone()))
        .route("/version", get(get_version));

    Router::new().nest("/api/v1", api_v1)
}

#[derive(Serialize)]
struct VersionResponse {
    version: &'static str,
}

async fn get_version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}

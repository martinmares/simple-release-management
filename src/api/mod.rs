pub mod bundles;
pub mod auth;
pub mod copy;
pub mod deploy;
pub mod git_repos;
pub mod argocd;
pub mod kubernetes;
pub mod registries;
pub mod releases;
pub mod tenants;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    let argocd_state = argocd::ArgocdApiState {
        pool: pool.clone(),
        encryption_secret: registry_state.encryption_secret.clone(),
        client_tls: reqwest::Client::builder()
            .build()
            .expect("Failed to build Argocd HTTP client"),
        client_insecure: reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("Failed to build Argocd HTTP client"),
        token_cache: Arc::new(RwLock::new(HashMap::new())),
    };
    let kubernetes_state = kubernetes::KubernetesApiState {
        pool: pool.clone(),
        encryption_secret: registry_state.encryption_secret.clone(),
        client_tls: reqwest::Client::builder()
            .build()
            .expect("Failed to build Kubernetes HTTP client"),
        client_insecure: reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("Failed to build Kubernetes HTTP client"),
        oauth_client_tls: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build Kubernetes OAuth client"),
        oauth_client_insecure: reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build Kubernetes OAuth client"),
        token_cache: Arc::new(RwLock::new(HashMap::new())),
    };

    let api_v1 = Router::new()
        .route("/auth/me", get(auth::me))
        .nest("/tenants", tenants::router(pool.clone()))
        .merge(registries::router(registry_state))
        .merge(git_repos::router(git_repo_state))
        .merge(argocd::router(argocd_state))
        .merge(kubernetes::router(kubernetes_state))
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

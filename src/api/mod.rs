pub mod bundles;
pub mod copy;
pub mod registries;
pub mod releases;
pub mod tenants;

use axum::Router;
use sqlx::PgPool;

/// Vytvoří router s všemi API endpointy
pub fn create_api_router(pool: PgPool) -> Router {
    let api_v1 = Router::new()
        .nest("/tenants", tenants::router(pool.clone()))
        .merge(registries::router(pool.clone()))
        .merge(bundles::router(pool.clone()))
        .merge(releases::router(pool.clone()));

    Router::new().nest("/api/v1", api_v1)
}

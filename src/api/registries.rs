use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::Registry;

/// Request pro vytvoření nové registry
#[derive(Debug, Deserialize)]
pub struct CreateRegistryRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub registry_type: String,
    pub base_url: String,
    pub credentials_path: String,
    pub role: String,
}

/// Request pro update registry
#[derive(Debug, Deserialize)]
pub struct UpdateRegistryRequest {
    pub name: String,
    pub registry_type: String,
    pub base_url: String,
    pub credentials_path: String,
    pub role: String,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Vytvoří router pro registries endpoints
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/registries", get(list_all_registries))
        .route("/tenants/{tenant_id}/registries", get(list_registries).post(create_registry))
        .route("/registries/{id}", get(get_registry).put(update_registry).delete(delete_registry))
        .with_state(pool)
}

/// GET /api/v1/registries - Seznam všech registries
async fn list_all_registries(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Registry>>, (StatusCode, Json<ErrorResponse>)> {
    let registries = sqlx::query_as::<_, Registry>(
        "SELECT * FROM registries ORDER BY created_at DESC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(registries))
}

/// GET /api/v1/tenants/{tenant_id}/registries - Seznam registries pro tenanta
async fn list_registries(
    State(pool): State<PgPool>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<Registry>>, (StatusCode, Json<ErrorResponse>)> {
    let registries = sqlx::query_as::<_, Registry>(
        "SELECT * FROM registries WHERE tenant_id = $1 ORDER BY created_at DESC"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(registries))
}

/// GET /api/v1/registries/{id} - Detail registry
async fn get_registry(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Registry>, (StatusCode, Json<ErrorResponse>)> {
    let registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    match registry {
        Some(registry) => Ok(Json(registry)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Registry with id {} not found", id),
            }),
        )),
    }
}

/// POST /api/v1/tenants/{tenant_id}/registries - Vytvoření nové registry
async fn create_registry(
    State(pool): State<PgPool>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<CreateRegistryRequest>,
) -> Result<(StatusCode, Json<Registry>), (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Registry name cannot be empty".to_string(),
            }),
        ));
    }

    if payload.base_url.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Registry base_url cannot be empty".to_string(),
            }),
        ));
    }

    // Validace registry_type
    let valid_types = ["harbor", "docker", "quay", "gcr", "ecr", "acr", "generic"];
    if !valid_types.contains(&payload.registry_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid registry_type. Must be one of: {}", valid_types.join(", ")),
            }),
        ));
    }

    // Validace role
    let valid_roles = ["source", "target", "both"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid role. Must be one of: {}", valid_roles.join(", ")),
            }),
        ));
    }

    // Zkontrolovat že tenant existuje
    let tenant_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM tenants WHERE id = $1)")
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    if !tenant_exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Tenant with id {} not found", tenant_id),
            }),
        ));
    }

    // Vytvoření registry
    let registry = sqlx::query_as::<_, Registry>(
        "INSERT INTO registries (tenant_id, name, registry_type, base_url, credentials_path, role)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, tenant_id, name, registry_type, base_url, credentials_path, role, created_at",
    )
    .bind(tenant_id)
    .bind(&payload.name)
    .bind(&payload.registry_type)
    .bind(&payload.base_url)
    .bind(&payload.credentials_path)
    .bind(&payload.role)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        // Zkontrolovat unique constraint violation
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("Registry with name '{}' already exists in this tenant", payload.name),
                    }),
                );
            }
        }

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(registry)))
}

/// PUT /api/v1/registries/{id} - Update registry
async fn update_registry(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRegistryRequest>,
) -> Result<Json<Registry>, (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Registry name cannot be empty".to_string(),
            }),
        ));
    }

    if payload.base_url.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Registry base_url cannot be empty".to_string(),
            }),
        ));
    }

    // Validace registry_type
    let valid_types = ["harbor", "docker", "quay", "gcr", "ecr", "acr", "generic"];
    if !valid_types.contains(&payload.registry_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid registry_type. Must be one of: {}", valid_types.join(", ")),
            }),
        ));
    }

    // Validace role
    let valid_roles = ["source", "target", "both"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid role. Must be one of: {}", valid_roles.join(", ")),
            }),
        ));
    }

    // Update registry
    let registry = sqlx::query_as::<_, Registry>(
        "UPDATE registries
         SET name = $1, registry_type = $2, base_url = $3, credentials_path = $4, role = $5
         WHERE id = $6
         RETURNING id, tenant_id, name, registry_type, base_url, credentials_path, role, created_at",
    )
    .bind(&payload.name)
    .bind(&payload.registry_type)
    .bind(&payload.base_url)
    .bind(&payload.credentials_path)
    .bind(&payload.role)
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        // Zkontrolovat unique constraint violation
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("Registry with name '{}' already exists in this tenant", payload.name),
                    }),
                );
            }
        }

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    match registry {
        Some(registry) => Ok(Json(registry)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Registry with id {} not found", id),
            }),
        )),
    }
}

/// DELETE /api/v1/registries/{id} - Smazání registry
async fn delete_registry(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM registries WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Registry with id {} not found", id),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

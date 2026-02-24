use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthContext;
use crate::db::models::Tenant;

/// Request pro vytvoření nového tenanta
#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Request pro update tenanta
#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Vytvoří router pro tenants endpoints
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_tenants).post(create_tenant))
        .route("/{id}", get(get_tenant).put(update_tenant).delete(delete_tenant))
        .with_state(pool)
}

/// GET /api/v1/tenants - Seznam všech tenantů
async fn list_tenants(
    Extension(auth): Extension<AuthContext>,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Tenant>>, (StatusCode, Json<ErrorResponse>)> {
    let tenants = if auth.is_admin() {
        sqlx::query_as::<_, Tenant>("SELECT * FROM tenants ORDER BY created_at DESC")
            .fetch_all(&pool)
            .await
    } else {
        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE id = ANY($1) ORDER BY created_at DESC",
        )
        .bind(&auth.tenant_ids)
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(tenants))
}

/// GET /api/v1/tenants/:id - Detail tenanta
async fn get_tenant(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Tenant>, (StatusCode, Json<ErrorResponse>)> {
    let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
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

    match tenant {
        Some(tenant) => Ok(Json(tenant)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Tenant with id {} not found", id),
            }),
        )),
    }
}

/// POST /api/v1/tenants - Vytvoření nového tenanta
async fn create_tenant(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<(StatusCode, Json<Tenant>), (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Tenant name cannot be empty".to_string(),
            }),
        ));
    }

    // Vytvoření tenanta
    let tenant = sqlx::query_as::<_, Tenant>(
        "INSERT INTO tenants (name, slug, description) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&payload.description)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        // Zkontrolovat unique constraint violation
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("Tenant with slug '{}' already exists", payload.slug),
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

    Ok((StatusCode::CREATED, Json(tenant)))
}

/// PUT /api/v1/tenants/:id - Update tenanta
async fn update_tenant(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTenantRequest>,
) -> Result<Json<Tenant>, (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Tenant name cannot be empty".to_string(),
            }),
        ));
    }

    // Update tenanta
    let tenant = sqlx::query_as::<_, Tenant>(
        "UPDATE tenants SET name = $1, description = $2 WHERE id = $3 RETURNING *",
    )
    .bind(&payload.name)
    .bind(&payload.description)
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
                        error: format!("Tenant with name '{}' already exists", payload.name),
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

    match tenant {
        Some(tenant) => Ok(Json(tenant)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Tenant with id {} not found", id),
            }),
        )),
    }
}

/// DELETE /api/v1/tenants/:id - Smazání tenanta
async fn delete_tenant(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM tenants WHERE id = $1")
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
                error: format!("Tenant with id {} not found", id),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

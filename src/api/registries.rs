use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto;
use crate::db::models::Registry;
use crate::services::skopeo::SkopeoCredentials;

#[derive(Clone)]
pub struct RegistryApiState {
    pub pool: PgPool,
    pub encryption_secret: String,
}

impl RegistryApiState {
    /// Získá dešifrované credentials pro registry
    pub async fn get_registry_credentials(
        &self,
        registry_id: Uuid,
    ) -> Result<Option<(String, String)>, anyhow::Error> {
        let registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
            .bind(registry_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(registry) = registry else {
            return Ok(None);
        };

        // Decrypt credentials based on auth_type
        match registry.auth_type.as_str() {
            "none" => Ok(None),
            "basic" => {
                let username = registry.username.clone().unwrap_or_default();
                let password = if let Some(encrypted) = &registry.password_encrypted {
                    crypto::decrypt(encrypted, &self.encryption_secret)?
                } else {
                    String::new()
                };
                Ok(Some((username, password)))
            }
            "token" => {
                let username = registry.username.clone().unwrap_or_default();
                let token = if let Some(encrypted) = &registry.token_encrypted {
                    crypto::decrypt(encrypted, &self.encryption_secret)?
                } else {
                    String::new()
                };
                Ok(Some((username, token)))
            }
            "bearer" => {
                let token = if let Some(encrypted) = &registry.token_encrypted {
                    crypto::decrypt(encrypted, &self.encryption_secret)?
                } else {
                    String::new()
                };
                // For bearer, username is empty but password contains the token
                Ok(Some((String::new(), token)))
            }
            _ => Ok(None),
        }
    }

    /// Vytvoří SkopeoCredentials pro copy operaci mezi source a target registry
    pub async fn get_skopeo_credentials(
        &self,
        source_registry_id: Uuid,
        target_registry_id: Uuid,
    ) -> Result<SkopeoCredentials, anyhow::Error> {
        let source_creds = self.get_registry_credentials(source_registry_id).await?;
        let target_creds = self.get_registry_credentials(target_registry_id).await?;

        let (source_username, source_password) = source_creds.unwrap_or_default();
        let (target_username, target_password) = target_creds.unwrap_or_default();

        Ok(SkopeoCredentials {
            source_username: if source_username.is_empty() {
                None
            } else {
                Some(source_username)
            },
            source_password: if source_password.is_empty() {
                None
            } else {
                Some(source_password)
            },
            target_username: if target_username.is_empty() {
                None
            } else {
                Some(target_username)
            },
            target_password: if target_password.is_empty() {
                None
            } else {
                Some(target_password)
            },
        })
    }
}

/// Request pro vytvoření nové registry
#[derive(Debug, Deserialize)]
pub struct CreateRegistryRequest {
    pub name: String,
    pub registry_type: String,
    pub base_url: String,
    pub default_project_path: Option<String>,
    pub auth_type: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub role: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub environment_paths: Option<Vec<EnvironmentRegistryPathInput>>,
}

/// Request pro update registry
#[derive(Debug, Deserialize)]
pub struct UpdateRegistryRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub registry_type: String,
    pub base_url: String,
    pub default_project_path: Option<String>,
    pub auth_type: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub role: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub environment_paths: Option<Vec<EnvironmentRegistryPathInput>>,
}

#[derive(Debug, Deserialize)]
pub struct EnvironmentRegistryPathInput {
    pub environment_id: Uuid,
    pub source_project_path_override: Option<String>,
    pub target_project_path_override: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EnvironmentRegistryPathView {
    pub environment_id: Uuid,
    pub env_name: String,
    pub env_slug: String,
    pub source_project_path_override: Option<String>,
    pub target_project_path_override: Option<String>,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Vytvoří router pro registries endpoints
pub fn router(state: RegistryApiState) -> Router {
    Router::new()
        .route("/registries", get(list_all_registries))
        .route(
            "/tenants/{tenant_id}/registries",
            get(list_registries).post(create_registry),
        )
        .route(
            "/registries/{id}",
            get(get_registry).put(update_registry).delete(delete_registry),
        )
        .route("/registries/{id}/environment-paths", get(get_registry_environment_paths))
        .with_state(state)
}

async fn upsert_environment_paths(
    pool: &PgPool,
    tenant_id: Uuid,
    registry_id: Uuid,
    paths: &[EnvironmentRegistryPathInput],
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if paths.is_empty() {
        return Ok(());
    }

    let env_ids: Vec<Uuid> = paths.iter().map(|p| p.environment_id).collect();
    let valid_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM environments WHERE tenant_id = $1 AND id = ANY($2)",
    )
    .bind(tenant_id)
    .bind(&env_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    if valid_ids.len() != env_ids.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment does not belong to tenant".to_string(),
            }),
        ));
    }

    for path in paths {
        let source_value = path
            .source_project_path_override
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.trim_matches('/').to_string());

        let target_value = path
            .target_project_path_override
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.trim_matches('/').to_string());

        let entries = [
            ("source", source_value),
            ("target", target_value),
        ];

        for (role, value) in entries {
            if let Some(project_path) = value {
                sqlx::query(
                    r#"
                    INSERT INTO environment_registry_paths (environment_id, registry_id, project_path_override, role)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (environment_id, registry_id, role)
                    DO UPDATE SET project_path_override = EXCLUDED.project_path_override
                    "#,
                )
                .bind(path.environment_id)
                .bind(registry_id)
                .bind(project_path)
                .bind(role)
                .execute(pool)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Database error: {}", e),
                        }),
                    )
                })?;
            } else {
                sqlx::query(
                    "DELETE FROM environment_registry_paths WHERE environment_id = $1 AND registry_id = $2 AND role = $3",
                )
                .bind(path.environment_id)
                .bind(registry_id)
                .bind(role)
                .execute(pool)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Database error: {}", e),
                        }),
                    )
                })?;
            }
        }
    }

    Ok(())
}

/// GET /api/v1/registries - Seznam všech registries
async fn list_all_registries(
    State(state): State<RegistryApiState>,
) -> Result<Json<Vec<Registry>>, (StatusCode, Json<ErrorResponse>)> {
    let registries = sqlx::query_as::<_, Registry>("SELECT * FROM registries ORDER BY created_at DESC")
        .fetch_all(&state.pool)
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
    State(state): State<RegistryApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<Registry>>, (StatusCode, Json<ErrorResponse>)> {
    let registries = sqlx::query_as::<_, Registry>(
        "SELECT * FROM registries WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
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
    State(state): State<RegistryApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Registry>, (StatusCode, Json<ErrorResponse>)> {
    let registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
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
    State(state): State<RegistryApiState>,
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
                error: format!(
                    "Invalid registry_type. Must be one of: {}",
                    valid_types.join(", ")
                ),
            }),
        ));
    }

    // Validace auth_type
    let valid_auth_types = ["none", "basic", "token", "bearer"];
    if !valid_auth_types.contains(&payload.auth_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "Invalid auth_type. Must be one of: {}",
                    valid_auth_types.join(", ")
                ),
            }),
        ));
    }

    // Validace auth credentials
    match payload.auth_type.as_str() {
        "basic" => {
            if payload.username.is_none() || payload.password.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "auth_type 'basic' requires username and password".to_string(),
                    }),
                ));
            }
        }
        "token" => {
            if payload.username.is_none() || payload.token.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "auth_type 'token' requires username and token".to_string(),
                    }),
                ));
            }
        }
        "bearer" => {
            if payload.token.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "auth_type 'bearer' requires token".to_string(),
                    }),
                ));
            }
        }
        _ => {}
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
    let tenant_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM tenants WHERE id = $1)")
            .bind(tenant_id)
            .fetch_one(&state.pool)
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

    // Encrypt password if provided
    let password_encrypted = if let Some(password) = &payload.password {
        Some(crypto::encrypt(password, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Encryption error: {}", e),
                }),
            )
        })?)
    } else {
        None
    };

    // Encrypt token if provided
    let token_encrypted = if let Some(token) = &payload.token {
        Some(crypto::encrypt(token, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Encryption error: {}", e),
                }),
            )
        })?)
    } else {
        None
    };

    let default_project_path = payload
        .default_project_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| path.trim_matches('/').to_string());

    // Vytvoření registry
    let registry = sqlx::query_as::<_, Registry>(
        "INSERT INTO registries (tenant_id, name, registry_type, base_url, default_project_path, auth_type, username, password_encrypted, token_encrypted, role, description, is_active)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(&payload.name)
    .bind(&payload.registry_type)
    .bind(&payload.base_url)
    .bind(&default_project_path)
    .bind(&payload.auth_type)
    .bind(&payload.username)
    .bind(&password_encrypted)
    .bind(&token_encrypted)
    .bind(&payload.role)
    .bind(&payload.description)
    .bind(payload.is_active.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        // Zkontrolovat unique constraint violation
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!(
                            "Registry with name '{}' already exists in this tenant",
                            payload.name
                        ),
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

    if let Some(paths) = payload.environment_paths.as_ref() {
        upsert_environment_paths(&state.pool, tenant_id, registry.id, paths).await?;
    }

    Ok((StatusCode::CREATED, Json(registry)))
}

/// PUT /api/v1/registries/{id} - Update registry
async fn update_registry(
    State(state): State<RegistryApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRegistryRequest>,
) -> Result<Json<Registry>, (StatusCode, Json<ErrorResponse>)> {
    let existing = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    let Some(existing) = existing else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Registry with id {} not found", id),
            }),
        ));
    };

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
                error: format!(
                    "Invalid registry_type. Must be one of: {}",
                    valid_types.join(", ")
                ),
            }),
        ));
    }

    // Validace auth_type
    let valid_auth_types = ["none", "basic", "token", "bearer"];
    if !valid_auth_types.contains(&payload.auth_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "Invalid auth_type. Must be one of: {}",
                    valid_auth_types.join(", ")
                ),
            }),
        ));
    }

    let username = payload
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| existing.username.clone());

    // Validace auth credentials
    match payload.auth_type.as_str() {
        "basic" => {
            if username.is_none()
                || (payload.password.is_none() && existing.password_encrypted.is_none())
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "auth_type 'basic' requires username and password".to_string(),
                    }),
                ));
            }
        }
        "token" => {
            if username.is_none() || (payload.token.is_none() && existing.token_encrypted.is_none())
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "auth_type 'token' requires username and token".to_string(),
                    }),
                ));
            }
        }
        "bearer" => {
            if payload.token.is_none() && existing.token_encrypted.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "auth_type 'bearer' requires token".to_string(),
                    }),
                ));
            }
        }
        _ => {}
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

    // Encrypt password if provided
    let password_encrypted = if let Some(password) = &payload.password {
        Some(crypto::encrypt(password, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Encryption error: {}", e),
                }),
            )
        })?)
    } else {
        existing.password_encrypted.clone()
    };

    // Encrypt token if provided
    let token_encrypted = if let Some(token) = &payload.token {
        Some(crypto::encrypt(token, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Encryption error: {}", e),
                }),
            )
        })?)
    } else {
        existing.token_encrypted.clone()
    };

    let default_project_path = payload
        .default_project_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| path.trim_matches('/').to_string());

    // Update registry
    let registry = sqlx::query_as::<_, Registry>(
        "UPDATE registries
         SET tenant_id = $1, name = $2, registry_type = $3, base_url = $4, default_project_path = $5, auth_type = $6, username = $7,
             password_encrypted = $8, token_encrypted = $9, role = $10, description = $11, is_active = $12
         WHERE id = $13
         RETURNING *",
    )
    .bind(&payload.tenant_id)
    .bind(&payload.name)
    .bind(&payload.registry_type)
    .bind(&payload.base_url)
    .bind(&default_project_path)
    .bind(&payload.auth_type)
    .bind(&username)
    .bind(&password_encrypted)
    .bind(&token_encrypted)
    .bind(&payload.role)
    .bind(&payload.description)
    .bind(payload.is_active.unwrap_or(true))
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        // Zkontrolovat unique constraint violation
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!(
                            "Registry with name '{}' already exists in this tenant",
                            payload.name
                        ),
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
        Some(registry) => {
            if let Some(paths) = payload.environment_paths.as_ref() {
                upsert_environment_paths(&state.pool, registry.tenant_id, registry.id, paths).await?;
            }
            Ok(Json(registry))
        }
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
    State(state): State<RegistryApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM registries WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
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

/// GET /api/v1/registries/{id}/environment-paths - Seznam env path overrides
async fn get_registry_environment_paths(
    State(state): State<RegistryApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<EnvironmentRegistryPathView>>, (StatusCode, Json<ErrorResponse>)> {
    let paths = sqlx::query_as::<_, EnvironmentRegistryPathView>(
        r#"
        SELECT
            e.id AS environment_id,
            e.name AS env_name,
            e.slug AS env_slug,
            erp_source.project_path_override AS source_project_path_override,
            erp_target.project_path_override AS target_project_path_override
        FROM environments e
        LEFT JOIN environment_registry_paths erp_source
          ON erp_source.environment_id = e.id AND erp_source.registry_id = $1 AND erp_source.role = 'source'
        LEFT JOIN environment_registry_paths erp_target
          ON erp_target.environment_id = e.id AND erp_target.registry_id = $1 AND erp_target.role = 'target'
        WHERE e.tenant_id = (SELECT tenant_id FROM registries WHERE id = $1)
        ORDER BY e.slug ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(Json(paths))
}

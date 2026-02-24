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
use crate::{
    crypto,
    db::models::GitRepository,
};

#[derive(Clone)]
pub struct GitRepoApiState {
    pub pool: PgPool,
    pub encryption_secret: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateGitRepoRequest {
    pub name: String,
    pub repo_url: String,
    pub default_branch: Option<String>,
    pub git_auth_type: String,
    pub git_username: Option<String>,
    pub git_token: Option<String>,
    pub git_ssh_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGitRepoRequest {
    pub name: String,
    pub repo_url: String,
    pub default_branch: Option<String>,
    pub git_auth_type: String,
    pub git_username: Option<String>,
    pub git_token: Option<String>,
    pub git_ssh_key: Option<String>,
}

pub fn router(state: GitRepoApiState) -> Router {
    Router::new()
        .route("/git-repos", get(list_git_repos))
        .route("/tenants/{tenant_id}/git-repos", get(list_tenant_git_repos).post(create_git_repo))
        .route("/git-repos/{id}", get(get_git_repo).put(update_git_repo).delete(delete_git_repo))
        .with_state(state)
}

async fn list_git_repos(
    Extension(auth): Extension<AuthContext>,
    State(state): State<GitRepoApiState>,
) -> Result<Json<Vec<GitRepository>>, (StatusCode, Json<ErrorResponse>)> {
    let repos = if auth.is_admin() {
        sqlx::query_as::<_, GitRepository>(
            "SELECT * FROM git_repositories ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, GitRepository>(
            "SELECT * FROM git_repositories WHERE tenant_id = ANY($1) ORDER BY created_at DESC",
        )
        .bind(&auth.tenant_ids)
        .fetch_all(&state.pool)
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

    Ok(Json(repos))
}

async fn list_tenant_git_repos(
    State(state): State<GitRepoApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<GitRepository>>, (StatusCode, Json<ErrorResponse>)> {
    let repos = sqlx::query_as::<_, GitRepository>(
        "SELECT * FROM git_repositories WHERE tenant_id = $1 ORDER BY created_at DESC",
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

    Ok(Json(repos))
}

async fn get_git_repo(
    State(state): State<GitRepoApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GitRepository>, (StatusCode, Json<ErrorResponse>)> {
    let repo = sqlx::query_as::<_, GitRepository>("SELECT * FROM git_repositories WHERE id = $1")
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

    match repo {
        Some(repo) => Ok(Json(repo)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Git repository with id {} not found", id),
            }),
        )),
    }
}

async fn create_git_repo(
    State(state): State<GitRepoApiState>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<CreateGitRepoRequest>,
) -> Result<(StatusCode, Json<GitRepository>), (StatusCode, Json<ErrorResponse>)> {
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name cannot be empty".to_string(),
            }),
        ));
    }

    if payload.repo_url.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Repo URL cannot be empty".to_string(),
            }),
        ));
    }

    if payload.git_auth_type == "token" && payload.git_token.as_deref().unwrap_or("").is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Git token is required for token auth".to_string(),
            }),
        ));
    }

    if payload.git_auth_type == "ssh" && payload.git_ssh_key.as_deref().unwrap_or("").is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "SSH key is required for SSH auth".to_string(),
            }),
        ));
    }

    let git_token_encrypted = match payload.git_token {
        Some(token) if !token.trim().is_empty() => Some(
            crypto::encrypt(&token, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt git token: {}", e),
                    }),
                )
            })?,
        ),
        _ => None,
    };

    let git_ssh_key_encrypted = match payload.git_ssh_key {
        Some(key) if !key.trim().is_empty() => Some(
            crypto::encrypt(&key, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt git ssh key: {}", e),
                    }),
                )
            })?,
        ),
        _ => None,
    };

    let repo = sqlx::query_as::<_, GitRepository>(
        r#"
        INSERT INTO git_repositories
            (tenant_id, name, repo_url, default_branch, git_auth_type, git_username, git_token_encrypted, git_ssh_key_encrypted)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#
    )
    .bind(tenant_id)
    .bind(payload.name.trim())
    .bind(payload.repo_url.trim())
    .bind(payload.default_branch.unwrap_or_else(|| "main".to_string()))
    .bind(payload.git_auth_type)
    .bind(payload.git_username)
    .bind(git_token_encrypted)
    .bind(git_ssh_key_encrypted)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create git repo: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(repo)))
}

async fn update_git_repo(
    State(state): State<GitRepoApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateGitRepoRequest>,
) -> Result<Json<GitRepository>, (StatusCode, Json<ErrorResponse>)> {
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name cannot be empty".to_string(),
            }),
        ));
    }

    if payload.repo_url.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Repo URL cannot be empty".to_string(),
            }),
        ));
    }

    let git_token_encrypted = match payload.git_token {
        Some(token) if !token.trim().is_empty() => Some(
            crypto::encrypt(&token, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt git token: {}", e),
                    }),
                )
            })?,
        ),
        _ => None,
    };

    let git_ssh_key_encrypted = match payload.git_ssh_key {
        Some(key) if !key.trim().is_empty() => Some(
            crypto::encrypt(&key, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt git ssh key: {}", e),
                    }),
                )
            })?,
        ),
        _ => None,
    };

    let repo = sqlx::query_as::<_, GitRepository>(
        r#"
        UPDATE git_repositories
        SET name = $1,
            repo_url = $2,
            default_branch = $3,
            git_auth_type = $4,
            git_username = $5,
            git_token_encrypted = COALESCE($6, git_token_encrypted),
            git_ssh_key_encrypted = COALESCE($7, git_ssh_key_encrypted)
        WHERE id = $8
        RETURNING *
        "#
    )
    .bind(payload.name.trim())
    .bind(payload.repo_url.trim())
    .bind(payload.default_branch.unwrap_or_else(|| "main".to_string()))
    .bind(payload.git_auth_type)
    .bind(payload.git_username)
    .bind(git_token_encrypted)
    .bind(git_ssh_key_encrypted)
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to update git repo: {}", e),
            }),
        )
    })?;

    Ok(Json(repo))
}

async fn delete_git_repo(
    State(state): State<GitRepoApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM git_repositories WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to delete git repo: {}", e),
                }),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Git repository with id {} not found", id),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

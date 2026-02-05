use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Sse},
    routing::{delete, get, post, put},
    Json, Router,
};
use anyhow::Context;
use futures::stream;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::Duration,
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::{broadcast, RwLock},
};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    crypto,
    db::models::{DeployJob, DeployJobDiff, DeployJobLog, DeployTarget, DeployTargetEncjsonKey, GitRepository, Release},
    services::release_manifest::build_release_manifest,
};

#[derive(Clone)]
pub struct DeployApiState {
    pub pool: PgPool,
    pub encryption_secret: String,
    pub kube_build_app_path: String,
    pub apply_env_path: String,
    pub encjson_path: String,
    pub kubeconform_path: String,
    pub job_logs: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeployTargetRequest {
    pub name: String,
    pub env_name: String,
    pub env_repo_id: Uuid,
    pub env_repo_path: Option<String>,
    pub deploy_repo_id: Uuid,
    pub deploy_repo_path: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub encjson_private_key: Option<String>,
    pub encjson_keys: Option<Vec<EncjsonKeyInput>>,
    pub allow_auto_release: Option<bool>,
    pub append_env_suffix: Option<bool>,
    pub is_active: Option<bool>,
    pub copy_from_target_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeployTargetRequest {
    pub name: String,
    pub env_name: String,
    pub env_repo_id: Uuid,
    pub env_repo_path: Option<String>,
    pub deploy_repo_id: Uuid,
    pub deploy_repo_path: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub encjson_private_key: Option<String>,
    pub encjson_keys: Option<Vec<EncjsonKeyInput>>,
    pub allow_auto_release: Option<bool>,
    pub append_env_suffix: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeployJobRequest {
    pub release_id: Uuid,
    pub deploy_target_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct AutoDeployFromCopyJobRequest {
    pub copy_job_id: Uuid,
    pub deploy_target_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EncjsonKeyInput {
    pub public_key: String,
    pub private_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeployTargetWithKeys {
    #[serde(flatten)]
    pub target: DeployTarget,
    pub encjson_keys: Vec<EncjsonKeySummary>,
}

#[derive(Debug, Serialize)]
pub struct EncjsonKeySummary {
    pub public_key: String,
    pub has_private: bool,
}

#[derive(Debug, Serialize)]
pub struct DeployJobResponse {
    pub job_id: Uuid,
    pub message: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeployJobSummary {
    pub id: Uuid,
    pub release_id: Uuid,
    pub deploy_target_id: Uuid,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
    pub commit_sha: Option<String>,
    pub tag_name: Option<String>,
    pub target_name: String,
    pub env_name: String,
    pub is_auto: bool,
    pub copy_job_id: Option<Uuid>,
    pub bundle_id: Option<Uuid>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeployJobListRow {
    pub id: Uuid,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
    pub commit_sha: Option<String>,
    pub tag_name: Option<String>,
    pub target_name: String,
    pub env_name: String,
    pub release_db_id: Uuid,
    pub release_id: String,
    pub is_auto: bool,
    pub bundle_id: Uuid,
    pub bundle_name: String,
    pub tenant_id: Uuid,
    pub tenant_name: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn router(state: DeployApiState) -> Router {
    Router::new()
        .route("/tenants/{tenant_id}/deploy-targets", get(list_deploy_targets).post(create_deploy_target))
        .route("/deploy-targets/{id}", get(get_deploy_target).put(update_deploy_target).delete(delete_deploy_target))
        .route("/releases/{id}/deploy-targets", get(list_release_deploy_targets))
        .route("/releases/{id}/deploy-jobs", get(list_release_deploy_jobs))
        .route("/deploy/jobs", get(list_deploy_jobs).post(create_deploy_job))
        .route("/deploy/jobs/from-copy", post(auto_deploy_from_copy_job))
        .route("/deploy/jobs/{id}", get(get_deploy_job))
        .route("/deploy/jobs/{id}/logs", get(deploy_job_logs_sse))
        .route("/deploy/jobs/{id}/logs/history", get(deploy_job_logs_history))
        .route("/deploy/jobs/{id}/diff", get(deploy_job_diff))
        .with_state(state)
}

async fn list_deploy_targets(
    State(state): State<DeployApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<DeployTarget>>, (StatusCode, Json<ErrorResponse>)> {
    let targets = sqlx::query_as::<_, DeployTarget>(
        "SELECT * FROM deploy_targets WHERE tenant_id = $1 ORDER BY created_at DESC",
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

    Ok(Json(targets))
}

async fn get_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeployTargetWithKeys>, (StatusCode, Json<ErrorResponse>)> {
    let target = sqlx::query_as::<_, DeployTarget>("SELECT * FROM deploy_targets WHERE id = $1")
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

    match target {
        Some(target) => {
            let keys = sqlx::query_as::<_, DeployTargetEncjsonKey>(
                "SELECT * FROM deploy_target_encjson_keys WHERE deploy_target_id = $1 ORDER BY created_at",
            )
            .bind(target.id)
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

            let summaries = keys
                .into_iter()
                .map(|k| EncjsonKeySummary {
                    public_key: k.public_key,
                    has_private: true,
                })
                .collect();

            Ok(Json(DeployTargetWithKeys {
                target,
                encjson_keys: summaries,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Deploy target with id {} not found", id),
            }),
        )),
    }
}

async fn create_deploy_target(
    State(state): State<DeployApiState>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<CreateDeployTargetRequest>,
) -> Result<(StatusCode, Json<DeployTarget>), (StatusCode, Json<ErrorResponse>)> {
    if payload.name.trim().is_empty() || payload.env_name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name and env name are required".to_string(),
            }),
        ));
    }
    // Validate git repositories belong to tenant
    let env_repo_ok = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(payload.env_repo_id)
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

    if !env_repo_ok {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment repository not found for tenant".to_string(),
            }),
        ));
    }

    let deploy_repo_ok = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(payload.deploy_repo_id)
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

    if !deploy_repo_ok {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy repository not found for tenant".to_string(),
            }),
        ));
    }

    let encjson_private_key_encrypted = match payload.encjson_private_key {
        Some(key) if !key.trim().is_empty() => Some(
            crypto::encrypt(&key, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt encjson private key: {}", e),
                    }),
                )
            })?,
        ),
        _ => None,
    };

    let env_repo_path = payload.env_repo_path.unwrap_or_else(|| payload.env_name.clone());
    let deploy_repo_path = payload
        .deploy_repo_path
        .unwrap_or_else(|| format!("deploy/{}", payload.env_name));

    let target = sqlx::query_as::<_, DeployTarget>(
        r#"
        INSERT INTO deploy_targets
        (tenant_id, name, env_name, env_repo_id, env_repo_path,
         deploy_repo_id, deploy_repo_path, deploy_path, encjson_key_dir, encjson_private_key_encrypted, allow_auto_release, append_env_suffix, is_active)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING *
        "#,
    )
    .bind(tenant_id)
    .bind(&payload.name)
    .bind(&payload.env_name)
    .bind(payload.env_repo_id)
    .bind(&env_repo_path)
    .bind(payload.deploy_repo_id)
    .bind(&deploy_repo_path)
    .bind(&deploy_repo_path)
    .bind(&payload.encjson_key_dir)
    .bind(&encjson_private_key_encrypted)
    .bind(payload.allow_auto_release.unwrap_or(false))
    .bind(payload.append_env_suffix.unwrap_or(false))
    .bind(payload.is_active.unwrap_or(true))
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

    if let Some(keys) = payload.encjson_keys {
        store_encjson_keys(&state, target.id, keys).await?;
    } else if let Some(source_id) = payload.copy_from_target_id {
        let same_tenant = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM deploy_targets WHERE id = $1 AND tenant_id = $2)",
        )
        .bind(source_id)
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

        if !same_tenant {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Copy source deploy target does not belong to this tenant".to_string(),
                }),
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO deploy_target_encjson_keys (deploy_target_id, public_key, private_key_encrypted)
            SELECT $1, public_key, private_key_encrypted
            FROM deploy_target_encjson_keys
            WHERE deploy_target_id = $2
            "#,
        )
        .bind(target.id)
        .bind(source_id)
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
    }

    Ok((StatusCode::CREATED, Json(target)))
}

async fn update_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateDeployTargetRequest>,
) -> Result<Json<DeployTarget>, (StatusCode, Json<ErrorResponse>)> {
    if payload.name.trim().is_empty() || payload.env_name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name and env name are required".to_string(),
            }),
        ));
    }
    let target_tenant = sqlx::query_scalar::<_, Uuid>(
        "SELECT tenant_id FROM deploy_targets WHERE id = $1",
    )
    .bind(id)
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

    let env_repo_ok = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(payload.env_repo_id)
    .bind(target_tenant)
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

    if !env_repo_ok {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment repository not found for tenant".to_string(),
            }),
        ));
    }

    let deploy_repo_ok = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(payload.deploy_repo_id)
    .bind(target_tenant)
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

    if !deploy_repo_ok {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy repository not found for tenant".to_string(),
            }),
        ));
    }

    let encjson_private_key_encrypted = match payload.encjson_private_key {
        Some(key) if !key.trim().is_empty() => Some(
            crypto::encrypt(&key, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt encjson private key: {}", e),
                    }),
                )
            })?,
        ),
        _ => None,
    };

    let env_repo_path = payload.env_repo_path.unwrap_or_else(|| payload.env_name.clone());
    let deploy_repo_path = payload
        .deploy_repo_path
        .unwrap_or_else(|| format!("deploy/{}", payload.env_name));

    let target = sqlx::query_as::<_, DeployTarget>(
        r#"
        UPDATE deploy_targets
        SET name = $1,
            env_name = $2,
            env_repo_id = $3,
            env_repo_path = $4,
            deploy_repo_id = $5,
            deploy_repo_path = $6,
            deploy_path = $7,
            encjson_key_dir = $8,
            encjson_private_key_encrypted = COALESCE($9, encjson_private_key_encrypted),
            allow_auto_release = $10,
            append_env_suffix = $11,
            is_active = $12
        WHERE id = $13
        RETURNING *
        "#,
    )
    .bind(&payload.name)
    .bind(&payload.env_name)
    .bind(payload.env_repo_id)
    .bind(&env_repo_path)
    .bind(payload.deploy_repo_id)
    .bind(&deploy_repo_path)
    .bind(&deploy_repo_path)
    .bind(&payload.encjson_key_dir)
    .bind(&encjson_private_key_encrypted)
    .bind(payload.allow_auto_release.unwrap_or(false))
    .bind(payload.append_env_suffix.unwrap_or(false))
    .bind(payload.is_active.unwrap_or(true))
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

    match target {
        Some(target) => {
            if let Some(keys) = payload.encjson_keys {
                store_encjson_keys(&state, target.id, keys).await?;
            }
            Ok(Json(target))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Deploy target with id {} not found", id),
            }),
        )),
    }
}

async fn delete_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let has_jobs = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM deploy_jobs WHERE deploy_target_id = $1)",
    )
    .bind(id)
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

    if has_jobs {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy target has deploy jobs and cannot be deleted".to_string(),
            }),
        ));
    }

    let result = sqlx::query("DELETE FROM deploy_targets WHERE id = $1")
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
                error: format!("Deploy target with id {} not found", id),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_release_deploy_targets(
    State(state): State<DeployApiState>,
    Path(release_id): Path<Uuid>,
) -> Result<Json<Vec<DeployTarget>>, (StatusCode, Json<ErrorResponse>)> {
    let targets = sqlx::query_as::<_, DeployTarget>(
        r#"
        SELECT dt.*
        FROM deploy_targets dt
        JOIN releases r ON r.id = $1
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        WHERE dt.tenant_id = b.tenant_id AND dt.is_active = TRUE
        ORDER BY dt.created_at DESC
        "#,
    )
    .bind(release_id)
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

    Ok(Json(targets))
}

async fn list_release_deploy_jobs(
    State(state): State<DeployApiState>,
    Path(release_id): Path<Uuid>,
) -> Result<Json<Vec<DeployJobSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let jobs = sqlx::query_as::<_, DeployJobSummary>(
        r#"
        SELECT dj.id, dj.release_id, dj.deploy_target_id, dj.status, dj.started_at, dj.completed_at,
               dj.error_message, dj.commit_sha, dj.tag_name, dt.name as target_name, dt.env_name,
               r.is_auto, r.copy_job_id, b.id as bundle_id
        FROM deploy_jobs dj
        JOIN deploy_targets dt ON dt.id = dj.deploy_target_id
        JOIN releases r ON r.id = dj.release_id
        LEFT JOIN copy_jobs cj ON cj.id = r.copy_job_id
        LEFT JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        LEFT JOIN bundles b ON b.id = bv.bundle_id
        WHERE dj.release_id = $1
        ORDER BY dj.created_at DESC
        "#,
    )
    .bind(release_id)
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

    Ok(Json(jobs))
}

async fn list_deploy_jobs(
    State(state): State<DeployApiState>,
) -> Result<Json<Vec<DeployJobListRow>>, (StatusCode, Json<ErrorResponse>)> {
    let jobs = sqlx::query_as::<_, DeployJobListRow>(
        r#"
        SELECT
            dj.id,
            dj.status,
            dj.started_at,
            dj.completed_at,
            dj.error_message,
            dj.commit_sha,
            dj.tag_name,
            dt.name as target_name,
            dt.env_name,
            r.id as release_db_id,
            r.release_id,
            r.is_auto,
            b.id as bundle_id,
            b.name as bundle_name,
            t.id as tenant_id,
            t.name as tenant_name
        FROM deploy_jobs dj
        JOIN deploy_targets dt ON dt.id = dj.deploy_target_id
        JOIN releases r ON r.id = dj.release_id
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        JOIN tenants t ON t.id = b.tenant_id
        ORDER BY dj.started_at DESC
        LIMIT 200
        "#
    )
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

    Ok(Json(jobs))
}

async fn get_deploy_job(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeployJobSummary>, (StatusCode, Json<ErrorResponse>)> {
    let job = sqlx::query_as::<_, DeployJobSummary>(
        r#"
        SELECT dj.id, dj.release_id, dj.deploy_target_id, dj.status, dj.started_at, dj.completed_at,
               dj.error_message, dj.commit_sha, dj.tag_name, dt.name as target_name, dt.env_name,
               r.is_auto, r.copy_job_id, b.id as bundle_id
        FROM deploy_jobs dj
        JOIN deploy_targets dt ON dt.id = dj.deploy_target_id
        JOIN releases r ON r.id = dj.release_id
        LEFT JOIN copy_jobs cj ON cj.id = r.copy_job_id
        LEFT JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        LEFT JOIN bundles b ON b.id = bv.bundle_id
        WHERE dj.id = $1
        "#,
    )
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

    match job {
        Some(job) => Ok(Json(job)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Deploy job with id {} not found", id),
            }),
        )),
    }
}

async fn create_deploy_job(
    State(state): State<DeployApiState>,
    Json(payload): Json<CreateDeployJobRequest>,
) -> Result<(StatusCode, Json<DeployJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let release_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM releases WHERE id = $1)",
    )
    .bind(payload.release_id)
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

    if !release_exists {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Release not found".to_string(),
            }),
        ));
    }

    let target_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM deploy_targets WHERE id = $1)",
    )
    .bind(payload.deploy_target_id)
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

    if !target_exists {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy target not found".to_string(),
            }),
        ));
    }

    let job_id = enqueue_deploy_job(&state, payload.release_id, payload.deploy_target_id).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(DeployJobResponse {
            job_id,
            message: "Deploy job started".to_string(),
        }),
    ))
}

async fn auto_deploy_from_copy_job(
    State(state): State<DeployApiState>,
    Json(payload): Json<AutoDeployFromCopyJobRequest>,
) -> Result<(StatusCode, Json<DeployJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let target = sqlx::query_as::<_, DeployTarget>(
        "SELECT * FROM deploy_targets WHERE id = $1",
    )
    .bind(payload.deploy_target_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy target not found".to_string(),
            }),
        )
    })?;

    if !target.allow_auto_release {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy target does not allow auto release".to_string(),
            }),
        ));
    }

    let job_row = sqlx::query_as::<_, (String, String, Uuid)>(
        r#"
        SELECT cj.status, cj.target_tag, b.tenant_id
        FROM copy_jobs cj
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        WHERE cj.id = $1
        "#,
    )
    .bind(payload.copy_job_id)
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

    let Some((status, target_tag, tenant_id)) = job_row else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Copy job with id {} not found", payload.copy_job_id),
            }),
        ));
    };

    if status != "success" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Copy job is not successful".to_string(),
            }),
        ));
    }

    if tenant_id != target.tenant_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy target does not belong to this tenant".to_string(),
            }),
        ));
    }

    let existing_release = sqlx::query_as::<_, Release>(
        "SELECT * FROM releases WHERE copy_job_id = $1",
    )
    .bind(payload.copy_job_id)
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

    let release = if let Some(release) = existing_release {
        release
    } else {
        let existing_by_tag = sqlx::query_as::<_, Release>(
            "SELECT * FROM releases WHERE release_id = $1",
        )
        .bind(&target_tag)
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

        if let Some(release) = existing_by_tag {
            release
        } else {
            sqlx::query_as::<_, Release>(
                "INSERT INTO releases (copy_job_id, release_id, status, notes, created_by, is_auto, auto_reason)
                 VALUES ($1, $2, 'draft', $3, $4, true, $5)
                 RETURNING id, copy_job_id, release_id, status, notes, created_by, is_auto, auto_reason, created_at",
            )
            .bind(payload.copy_job_id)
            .bind(&target_tag)
            .bind("Auto release from copy job")
            .bind("system")
            .bind("copy_job_deploy")
            .fetch_one(&state.pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to create auto release: {}", e),
                    }),
                )
            })?
        }
    };

    let job_id = enqueue_deploy_job(&state, release.id, payload.deploy_target_id).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(DeployJobResponse {
            job_id,
            message: "Deploy job started".to_string(),
        }),
    ))
}

async fn enqueue_deploy_job(
    state: &DeployApiState,
    release_id: Uuid,
    deploy_target_id: Uuid,
) -> Result<Uuid, (StatusCode, Json<ErrorResponse>)> {
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO deploy_jobs (id, release_id, deploy_target_id, status) VALUES ($1, $2, $3, 'pending')",
    )
    .bind(job_id)
    .bind(release_id)
    .bind(deploy_target_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create deploy job: {}", e),
            }),
        )
    })?;

    let (log_tx, _log_rx) = broadcast::channel(512);
    state.job_logs.write().await.insert(job_id, log_tx.clone());

    let state_clone = state.clone();
    let log_persist_state = state.clone();
    let mut log_rx = log_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(line) = log_rx.recv().await {
            let _ = sqlx::query(
                "INSERT INTO deploy_job_logs (deploy_job_id, log_line) VALUES ($1, $2)",
            )
            .bind(job_id)
            .bind(line)
            .execute(&log_persist_state.pool)
            .await;
        }
    });

    tokio::spawn(async move {
        if let Err(e) = run_deploy_job(state_clone.clone(), job_id, log_tx.clone()).await {
            let _ = log_tx.send(format!("Deploy job failed: {}", e));
            let _ = sqlx::query(
                "UPDATE deploy_jobs SET status = 'failed', completed_at = NOW(), error_message = $1 WHERE id = $2",
            )
            .bind(e.to_string())
            .bind(job_id)
            .execute(&state_clone.pool)
            .await;
        }
    });

    Ok(job_id)
}

async fn store_encjson_keys(
    state: &DeployApiState,
    deploy_target_id: Uuid,
    keys: Vec<EncjsonKeyInput>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let mut existing: HashMap<String, String> = HashMap::new();
    let rows = sqlx::query_as::<_, DeployTargetEncjsonKey>(
        "SELECT * FROM deploy_target_encjson_keys WHERE deploy_target_id = $1",
    )
    .bind(deploy_target_id)
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

    for row in rows {
        existing.insert(row.public_key, row.private_key_encrypted);
    }

    let mut resolved = Vec::new();
    for key in keys {
        let public = key.public_key.trim().to_string();
        if public.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Encjson public key cannot be empty".to_string(),
                }),
            ));
        }
        let encrypted = if let Some(private) = key.private_key.as_deref().filter(|v| !v.trim().is_empty()) {
            crypto::encrypt(private, &state.encryption_secret).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to encrypt encjson private key: {}", e),
                    }),
                )
            })?
        } else if let Some(existing_enc) = existing.get(&public) {
            existing_enc.clone()
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Private key missing for public key {}", public),
                }),
            ));
        };

        resolved.push((public, encrypted));
    }

    let mut tx = state.pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    sqlx::query("DELETE FROM deploy_target_encjson_keys WHERE deploy_target_id = $1")
        .bind(deploy_target_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    for (public, encrypted) in resolved {
        sqlx::query(
            "INSERT INTO deploy_target_encjson_keys (deploy_target_id, public_key, private_key_encrypted)\n             VALUES ($1, $2, $3)",
        )
        .bind(deploy_target_id)
        .bind(&public)
        .bind(&encrypted)
        .execute(&mut *tx)
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

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    Ok(())
}

async fn run_deploy_job(state: DeployApiState, job_id: Uuid, log_tx: broadcast::Sender<String>) -> anyhow::Result<()> {
    let _ = log_tx.send(format!("Starting deploy job {}", job_id));

    sqlx::query("UPDATE deploy_jobs SET status = 'in_progress' WHERE id = $1")
        .bind(job_id)
        .execute(&state.pool)
        .await?;

    let job = sqlx::query_as::<_, DeployJob>("SELECT * FROM deploy_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&state.pool)
        .await?;

    let target = sqlx::query_as::<_, DeployTarget>("SELECT * FROM deploy_targets WHERE id = $1")
        .bind(job.deploy_target_id)
        .fetch_one(&state.pool)
        .await?;

    let release = sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = $1")
        .bind(job.release_id)
        .fetch_one(&state.pool)
        .await?;

    let temp_dir = TempDir::new()?;
    let env_repo_path = temp_dir.path().join("environments");
    let deploy_repo_path = temp_dir.path().join("deploy");

    let env_repo_id = target.env_repo_id.ok_or_else(|| anyhow::anyhow!("Deploy target missing env_repo_id"))?;
    let deploy_repo_id = target.deploy_repo_id.ok_or_else(|| anyhow::anyhow!("Deploy target missing deploy_repo_id"))?;

    let env_repo = sqlx::query_as::<_, GitRepository>("SELECT * FROM git_repositories WHERE id = $1")
        .bind(env_repo_id)
        .fetch_one(&state.pool)
        .await?;
    let deploy_repo = sqlx::query_as::<_, GitRepository>("SELECT * FROM git_repositories WHERE id = $1")
        .bind(deploy_repo_id)
        .fetch_one(&state.pool)
        .await?;

    let git_env_env = build_git_env_for_repo(&state, &env_repo, temp_dir.path())?;
    let git_env_deploy = build_git_env_for_repo(&state, &deploy_repo, temp_dir.path())?;

    run_git_clone(&env_repo.repo_url, &env_repo.default_branch, &env_repo_path, &git_env_env, &log_tx).await?;
    run_git_clone(&deploy_repo.repo_url, &deploy_repo.default_branch, &deploy_repo_path, &git_env_deploy, &log_tx).await?;

    let release_manifest = build_release_manifest(&state.pool, release.id).await?;
    let manifest_path = temp_dir.path().join("release-manifest.yml");
    let yaml = serde_yaml_ng::to_string(&release_manifest)?;
    tokio::fs::write(&manifest_path, yaml)
        .await
        .with_context(|| format!("Failed to write release manifest to {}", manifest_path.display()))?;

    let deploy_rel_path = target
        .deploy_repo_path
        .as_deref()
        .unwrap_or("")
        .trim()
        .trim_start_matches('/');
    let deploy_path = if deploy_rel_path.is_empty() {
        deploy_repo_path.clone()
    } else {
        deploy_repo_path.join(deploy_rel_path)
    };
    if !deploy_path.exists() {
        tokio::fs::create_dir_all(&deploy_path)
            .await
            .with_context(|| format!("Failed to create deploy path {}", deploy_path.display()))?;
    }

    clean_deploy_output(&deploy_path).await?;

    let kube_build_env = build_kube_build_env(&target, &release, &manifest_path, &env_repo_path)?;
    run_command_logged(
        &state.kube_build_app_path,
        &["-e", &target.env_name, "-t", deploy_path.to_string_lossy().as_ref(), "-r", manifest_path.to_string_lossy().as_ref()],
        Some(&env_repo_path),
        &kube_build_env,
        &log_tx,
        "kube_build_app",
    )
    .await?;

    run_command_logged(
        &state.kube_build_app_path,
        &["-e", &target.env_name, "-s"],
        Some(&env_repo_path),
        &kube_build_env,
        &log_tx,
        "kube_build_app -s",
    )
    .await?;

    let env_file_path = temp_dir.path().join("release.env");
    build_env_file(&state, &target, &env_repo_path, &env_file_path, &release, &log_tx, temp_dir.path()).await?;

    apply_env_to_outputs(&state, &deploy_path, &env_file_path, &log_tx).await?;

    let kubeconform_path = state.kubeconform_path.trim();
    if kubeconform_path.is_empty() {
        let _ = log_tx.send("kubeconform skipped (KUBECONFORM_PATH not set)".to_string());
    } else if let Err(err) = run_command_logged(
        kubeconform_path,
        &["-strict", "-summary", "-output", "json", "."],
        Some(&deploy_path),
        &HashMap::new(),
        &log_tx,
        "kubeconform",
    )
    .await
    {
        let not_found = err
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .map(|e| e.kind() == ErrorKind::NotFound)
            .unwrap_or(false);

        if not_found {
            let _ = log_tx.send("kubeconform not found, skipping validation".to_string());
        } else {
            let _ = log_tx.send("kubeconform reported errors (ignored)".to_string());
        }
    }

    let diff_info = collect_deploy_diff(&deploy_repo_path, deploy_rel_path, &log_tx).await?;
    let tag_name = if target.append_env_suffix {
        format!("{}-{}", release.release_id, target.env_name)
    } else {
        release.release_id.clone()
    };

    if let Some(diff) = diff_info {
    run_git_commit_and_push(
        &deploy_repo_path,
        deploy_rel_path,
        &tag_name,
        &deploy_repo.repo_url,
        &git_env_deploy,
        &log_tx,
    )
        .await?;

        let _ = sqlx::query(
            "INSERT INTO deploy_job_diffs (deploy_job_id, files_changed, diff_patch) VALUES ($1, $2, $3)",
        )
        .bind(job_id)
        .bind(diff.files_changed)
        .bind(diff.diff_patch)
        .execute(&state.pool)
        .await;
    } else {
        let _ = log_tx.send("No deploy changes detected; skipping git commit/push/tag".to_string());
    }

    let commit_sha = get_git_head_sha(&deploy_repo_path, &git_env_deploy).await.ok();

    sqlx::query(
        "UPDATE deploy_jobs SET status = 'success', completed_at = NOW(), commit_sha = $1, tag_name = $2 WHERE id = $3",
    )
    .bind(&commit_sha)
    .bind(&tag_name)
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    let _ = log_tx.send("Deploy job completed successfully".to_string());
    Ok(())
}

fn build_git_env_for_repo(
    state: &DeployApiState,
    repo: &GitRepository,
    temp_root: &FsPath,
) -> anyhow::Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());

    match repo.git_auth_type.as_str() {
        "ssh" => {
            if let Some(enc_key) = &repo.git_ssh_key_encrypted {
                let mut key = crypto::decrypt(enc_key, &state.encryption_secret)?;
                // Normalize key formatting in case it was stored with escaped newlines.
                if key.contains("\\n") {
                    key = key.replace("\\n", "\n");
                }
                if key.contains("\r\n") {
                    key = key.replace("\r\n", "\n");
                }
                let key = if key.ends_with('\n') { key } else { format!("{key}\n") };
                let key_path = temp_root.join(format!("git_ssh_key_{}", repo.id));
                std::fs::write(&key_path, key.as_bytes())
                    .with_context(|| format!("Failed to write git ssh key {}", key_path.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&key_path)?.permissions();
                    perms.set_mode(0o600);
                    std::fs::set_permissions(&key_path, perms)?;
                }
                let ssh_cmd = format!(
                    "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
                    key_path.display()
                );
                env.insert("GIT_SSH_COMMAND".to_string(), ssh_cmd);
            }
        }
        "token" => {
            if let (Some(enc_token), Some(username)) = (&repo.git_token_encrypted, &repo.git_username) {
                let token = crypto::decrypt(enc_token, &state.encryption_secret)?;
                env.insert("SRM_GIT_TOKEN".to_string(), token);
                env.insert("SRM_GIT_USERNAME".to_string(), username.clone());
            }
        }
        _ => {}
    }

    Ok(env)
}

fn inject_http_auth(repo_url: &str, username: &str, token: &str) -> anyhow::Result<String> {
    let mut url = url::Url::parse(repo_url)?;
    url.set_username(username).ok();
    url.set_password(Some(token)).ok();
    Ok(url.to_string())
}

async fn run_git_clone(
    repo_url: &str,
    branch: &str,
    path: &FsPath,
    git_env: &HashMap<String, String>,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let _ = log_tx.send(format!("Cloning {} (branch {})", repo_url, branch));

    let mut url = repo_url.to_string();
    if let Some(token) = git_env.get("SRM_GIT_TOKEN") {
        if let Some(username) = git_env.get("SRM_GIT_USERNAME") {
            url = inject_http_auth(&url, username, token)?;
        }
    }

    run_command_logged(
        "git",
        &["clone", "--branch", branch, &url, path.to_string_lossy().as_ref()],
        None,
        git_env,
        log_tx,
        "git clone",
    )
    .await
}

fn build_kube_build_env(
    target: &DeployTarget,
    release: &Release,
    manifest_path: &FsPath,
    env_repo_path: &FsPath,
) -> anyhow::Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    env.insert(
        "ENVIRONMENTS_DIR".to_string(),
        env_repo_path.to_string_lossy().to_string(),
    );
    env.insert("TSM_RELEASE_ID".to_string(), release.release_id.clone());
    env.insert(
        "SRM_RELEASE_MANIFEST".to_string(),
        manifest_path.to_string_lossy().to_string(),
    );
    env.insert("BUILD_ENV".to_string(), target.env_name.clone());
    Ok(env)
}

async fn build_env_file(
    state: &DeployApiState,
    target: &DeployTarget,
    env_repo_path: &FsPath,
    env_file_path: &FsPath,
    release: &Release,
    log_tx: &broadcast::Sender<String>,
    temp_root: &FsPath,
) -> anyhow::Result<()> {
    let env_subdir = target
        .env_repo_path
        .as_deref()
        .unwrap_or(&target.env_name);
    let env_dir = env_repo_path.join(env_subdir);
    let secured = env_dir.join("env.secured.json");
    let unsecured = env_dir.join("env.unsecured.json");

    let mut combined = String::new();

    let temp_key_dir = build_encjson_keydir(state, target, temp_root).await?;
    let key_dir_override = temp_key_dir.as_ref().map(|p| p.as_path());

    if secured.exists() {
        let output = run_encjson_dotenv(state, target, &secured, log_tx, key_dir_override).await?;
        combined.push_str(&output);
    }

    if unsecured.exists() {
        let output = run_encjson_dotenv(state, target, &unsecured, log_tx, key_dir_override).await?;
        combined.push_str(&output);
    }

    combined.push_str(&format!("TSM_RELEASE_ID={}\n", release.release_id));

    tokio::fs::write(env_file_path, combined)
        .await
        .with_context(|| format!("Failed to write env file {}", env_file_path.display()))?;
    Ok(())
}

async fn run_encjson_dotenv(
    state: &DeployApiState,
    target: &DeployTarget,
    file_path: &FsPath,
    log_tx: &broadcast::Sender<String>,
    keydir_override: Option<&FsPath>,
) -> anyhow::Result<String> {
    let mut cmd = Command::new(&state.encjson_path);
    cmd.arg("decrypt")
        .arg("-f")
        .arg(file_path)
        .arg("-o")
        .arg("dot-env");

    if let Some(keydir) = keydir_override {
        cmd.arg("-k").arg(keydir);
    } else if let Some(keydir) = &target.encjson_key_dir {
        cmd.arg("-k").arg(keydir);
    }

    if let Some(enc_key) = &target.encjson_private_key_encrypted {
        let key = crypto::decrypt(enc_key, &state.encryption_secret)?;
        cmd.env("ENCJSON_PRIVATE_KEY", key);
    }

    let output = cmd.output().await?;
    if !output.status.success() {
        let _ = log_tx.send(format!("encjson failed for {}", file_path.display()));
        anyhow::bail!("encjson failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn build_encjson_keydir(
    state: &DeployApiState,
    target: &DeployTarget,
    temp_root: &FsPath,
) -> anyhow::Result<Option<PathBuf>> {
    if target.encjson_key_dir.as_deref().unwrap_or("").is_empty() {
        let keys = sqlx::query_as::<_, DeployTargetEncjsonKey>(
            "SELECT * FROM deploy_target_encjson_keys WHERE deploy_target_id = $1 ORDER BY created_at",
        )
        .bind(target.id)
        .fetch_all(&state.pool)
        .await?;

        if keys.is_empty() {
            return Ok(None);
        }

        let key_dir = temp_root.join("encjson_keys");
        tokio::fs::create_dir_all(&key_dir)
            .await
            .with_context(|| format!("Failed to create encjson key dir {}", key_dir.display()))?;
        for key in keys {
            let private = crypto::decrypt(&key.private_key_encrypted, &state.encryption_secret)?;
            let file_path = key_dir.join(&key.public_key);
            tokio::fs::write(&file_path, private)
                .await
                .with_context(|| format!("Failed to write encjson key {}", file_path.display()))?;
        }
        return Ok(Some(key_dir));
    }

    Ok(None)
}

async fn apply_env_to_outputs(
    state: &DeployApiState,
    deploy_path: &FsPath,
    env_file_path: &FsPath,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let deployments = deploy_path.join("deployments");
    let services_external = deploy_path.join("services").join("external");

    let _ = log_tx.send("Applying env to deployments".to_string());
    apply_env_to_dir(state, &deployments, env_file_path, log_tx).await?;

    if services_external.exists() {
        let _ = log_tx.send("Applying env to services/external".to_string());
        apply_env_to_dir(state, &services_external, env_file_path, log_tx).await?;
    }

    Ok(())
}

async fn apply_env_to_dir(
    state: &DeployApiState,
    dir: &FsPath,
    env_file_path: &FsPath,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) == Some("yml") {
                run_command_logged(
                    &state.apply_env_path,
                    &["-E", env_file_path.to_string_lossy().as_ref(), "-f", path.to_string_lossy().as_ref(), "-w"],
                    None,
                    &HashMap::new(),
                    log_tx,
                    "apply-env",
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn run_git_commit_and_push(
    repo_path: &FsPath,
    deploy_path: &str,
    release_id: &str,
    repo_url: &str,
    git_env: &HashMap<String, String>,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let _ = log_tx.send("Preparing git commit".to_string());

    run_command_logged("git", &["config", "user.name", "simple-release-management"], Some(repo_path), git_env, log_tx, "git config").await?;
    run_command_logged("git", &["config", "user.email", "release-management@local"], Some(repo_path), git_env, log_tx, "git config").await?;

    let add_path = if deploy_path.trim().is_empty() { "." } else { deploy_path };
    run_command_logged("git", &["add", add_path], Some(repo_path), git_env, log_tx, "git add").await?;

    let commit_msg = format!("release {}", release_id);
    run_command_logged(
        "git",
        &["commit", "--allow-empty", "-m", &commit_msg],
        Some(repo_path),
        git_env,
        log_tx,
        "git commit",
    )
    .await?;

    run_command_logged(
        "git",
        &["tag", "-f", "-a", release_id, "-m", &commit_msg],
        Some(repo_path),
        git_env,
        log_tx,
        "git tag",
    )
    .await?;

    if let (Some(token), Some(username)) = (git_env.get("SRM_GIT_TOKEN"), git_env.get("SRM_GIT_USERNAME")) {
        let authed = inject_http_auth(repo_url, username, token)?;
        run_command_logged(
            "git",
            &["remote", "set-url", "origin", &authed],
            Some(repo_path),
            git_env,
            log_tx,
            "git remote set-url",
        )
        .await?;
    }

    run_command_logged("git", &["push"], Some(repo_path), git_env, log_tx, "git push").await?;
    run_command_logged("git", &["push", "--force", "--tags"], Some(repo_path), git_env, log_tx, "git push --tags").await?;

    Ok(())
}

async fn get_git_head_sha(repo_path: &FsPath, git_env: &HashMap<String, String>) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(repo_path)
        .envs(git_env)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!("git rev-parse failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn clean_deploy_output(deploy_path: &FsPath) -> anyhow::Result<()> {
    let assets = deploy_path.join("assets");
    let deployments = deploy_path.join("deployments");
    let services = deploy_path.join("services");

    if assets.exists() {
        tokio::fs::remove_dir_all(&assets).await.ok();
    }
    if deployments.exists() {
        tokio::fs::remove_dir_all(&deployments).await.ok();
    }
    if services.exists() {
        tokio::fs::remove_dir_all(&services).await.ok();
    }

    Ok(())
}

async fn run_command_logged(
    program: &str,
    args: &[&str],
    cwd: Option<&FsPath>,
    envs: &HashMap<String, String>,
    log_tx: &broadcast::Sender<String>,
    label: &str,
) -> anyhow::Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.envs(envs);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let log_tx_clone = log_tx.clone();
    let stdout_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            let _ = log_tx_clone.send(line);
        }
    });

    let log_tx_clone = log_tx.clone();
    let stderr_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            let _ = log_tx_clone.send(line);
        }
    });

    let status = child.wait().await?;
    stdout_task.await.ok();
    stderr_task.await.ok();

    if !status.success() {
        let _ = log_tx.send(format!("{} failed with exit code {:?}", label, status.code()));
        anyhow::bail!("{} failed", label);
    }

    Ok(())
}

struct DeployDiffSnapshot {
    files_changed: String,
    diff_patch: String,
}

async fn collect_deploy_diff(
    repo_path: &FsPath,
    deploy_path: &str,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<Option<DeployDiffSnapshot>> {
    let add_path = if deploy_path.trim().is_empty() { "." } else { deploy_path };

    let status_out = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .arg("--")
        .arg(add_path)
        .current_dir(repo_path)
        .output()
        .await?;
    if !status_out.status.success() {
        let _ = log_tx.send("git status failed".to_string());
        return Ok(None);
    }
    let files_changed = String::from_utf8_lossy(&status_out.stdout).trim().to_string();
    if files_changed.is_empty() {
        let _ = log_tx.send("No deploy changes detected".to_string());
        return Ok(None);
    }

    let diff_out = Command::new("git")
        .arg("diff")
        .arg("--unified=3")
        .arg("--")
        .arg(add_path)
        .current_dir(repo_path)
        .output()
        .await?;
    if !diff_out.status.success() {
        let _ = log_tx.send("git diff failed".to_string());
        return Ok(None);
    }
    let diff_patch = String::from_utf8_lossy(&diff_out.stdout).to_string();

    Ok(Some(DeployDiffSnapshot { files_changed, diff_patch }))
}

async fn deploy_job_logs_sse(
    State(state): State<DeployApiState>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    let receiver = {
        let logs = state.job_logs.read().await;
        logs.get(&job_id).map(|sender| sender.subscribe())
    };

    let Some(mut rx) = receiver else {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "Log stream not available",
        )
            .into_response();
    };

    let stream = stream::unfold(rx, |mut rx| async {
        match rx.recv().await {
            Ok(msg) => Some((Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(msg)), rx)),
            Err(_) => None,
        }
    });

    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response()
}

async fn deploy_job_logs_history(
    State(state): State<DeployApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    let rows = sqlx::query_as::<_, DeployJobLog>(
        "SELECT * FROM deploy_job_logs WHERE deploy_job_id = $1 ORDER BY created_at",
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to load deploy job logs: {}", e),
            }),
        )
    })?;

    let lines = rows.into_iter().map(|row| row.log_line).collect();
    Ok(Json(lines))
}

async fn deploy_job_diff(
    State(state): State<DeployApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Option<DeployJobDiff>>, (StatusCode, Json<ErrorResponse>)> {
    let row = sqlx::query_as::<_, DeployJobDiff>(
        "SELECT * FROM deploy_job_diffs WHERE deploy_job_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to load deploy job diff: {}", e),
            }),
        )
    })?;

    Ok(Json(row))
}

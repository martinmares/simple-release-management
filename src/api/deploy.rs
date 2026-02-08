#![allow(dead_code)]

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use anyhow::Context;
use futures::stream;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value as YamlValue;
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet},
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
    db::models::{
        DeployJob, DeployJobDiff, DeployJobLog, DeployTarget, DeployTargetEncjsonKey, DeployTargetEnv,
        DeployTargetEnvVar, DeployTargetExtraEnvVar, Environment, GitRepository, Release,
    },
    services::release_manifest::{build_release_manifest, ReleaseManifest},
};

async fn ensure_environment(
    pool: &PgPool,
    tenant_id: Uuid,
    env_name: &str,
) -> Result<Environment, (StatusCode, Json<ErrorResponse>)> {
    let name = env_name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment name cannot be empty".to_string(),
            }),
        ));
    }
    let slug = slugify_env_name(name);

    let env = sqlx::query_as::<_, Environment>(
        r#"
        INSERT INTO environments (tenant_id, name, slug)
        VALUES ($1, $2, $3)
        ON CONFLICT (tenant_id, slug)
        DO UPDATE SET name = EXCLUDED.name
        RETURNING *
        "#
    )
    .bind(tenant_id)
    .bind(name)
    .bind(&slug)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to upsert environment: {}", e),
            }),
        )
    })?;

    Ok(env)
}

fn slugify_env_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !last_dash && !out.is_empty() {
                out.push('-');
                last_dash = true;
            }
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

fn sanitize_path(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().trim_matches('/').to_string())
        .filter(|v| !v.is_empty())
}

fn env_vars_to_json(vars: Option<Vec<DeployTargetEnvVarInput>>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(items) = vars {
        for item in items {
            let key = item.source_key.trim();
            let value = item.target_key.trim();
            if !key.is_empty() && !value.is_empty() {
                map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
            }
        }
    }
    serde_json::Value::Object(map)
}

fn extra_env_vars_to_json(vars: Option<Vec<DeployTargetExtraEnvVarInput>>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(items) = vars {
        for item in items {
            let key = item.key.trim();
            let value = item.value.trim();
            if !key.is_empty() {
                map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
            }
        }
    }
    serde_json::Value::Object(map)
}

fn env_vars_from_json(value: &serde_json::Value) -> Vec<DeployTargetEnvVarInput> {
    let mut items = Vec::new();
    if let serde_json::Value::Object(map) = value {
        for (source_key, target_val) in map {
            let target_key = target_val.as_str().unwrap_or("").to_string();
            if !source_key.trim().is_empty() && !target_key.trim().is_empty() {
                items.push(DeployTargetEnvVarInput {
                    source_key: source_key.to_string(),
                    target_key,
                });
            }
        }
    }
    items
}

fn extra_env_vars_from_json(value: &serde_json::Value) -> Vec<DeployTargetExtraEnvVarInput> {
    let mut items = Vec::new();
    if let serde_json::Value::Object(map) = value {
        for (key, val) in map {
            let value = val.as_str().unwrap_or("").to_string();
            if !key.trim().is_empty() {
                items.push(DeployTargetExtraEnvVarInput {
                    key: key.to_string(),
                    value,
                });
            }
        }
    }
    items
}

async fn upsert_deploy_target_env(
    pool: &PgPool,
    deploy_target_id: Uuid,
    environment: &Environment,
    payload_env_repo_id: Uuid,
    payload_env_repo_path: Option<String>,
    payload_env_repo_branch: Option<String>,
    payload_deploy_repo_id: Uuid,
    payload_deploy_repo_path: Option<String>,
    payload_deploy_repo_branch: Option<String>,
    allow_auto_release: bool,
    append_env_suffix: bool,
    is_active: bool,
    release_manifest_mode: Option<String>,
    encjson_key_dir: Option<String>,
) -> Result<DeployTargetEnv, (StatusCode, Json<ErrorResponse>)> {
    let env_repo_branch = payload_env_repo_branch
        .clone()
        .filter(|v| !v.trim().is_empty());
    let deploy_repo_branch = payload_deploy_repo_branch
        .clone()
        .filter(|v| !v.trim().is_empty());
    let env_repo_path = payload_env_repo_path
        .clone()
        .filter(|v| !v.trim().is_empty());
    let deploy_repo_path = payload_deploy_repo_path
        .clone()
        .filter(|v| !v.trim().is_empty());
    let env_repo_path = if env_repo_branch.is_some() {
        env_repo_path
    } else {
        Some(env_repo_path.unwrap_or_else(|| environment.slug.clone()))
    };
    let deploy_repo_path = if deploy_repo_branch.is_some() {
        deploy_repo_path
    } else {
        Some(deploy_repo_path.unwrap_or_else(|| format!("deploy/{}", environment.slug)))
    };
    let manifest_mode = release_manifest_mode.unwrap_or_else(|| "match_digest".to_string());

    let row = sqlx::query_as::<_, DeployTargetEnv>(
        r#"
        INSERT INTO deploy_target_envs
            (deploy_target_id, environment_id, env_repo_id, env_repo_path, env_repo_branch,
             deploy_repo_id, deploy_repo_path, deploy_repo_branch,
             allow_auto_release, append_env_suffix, is_active, release_manifest_mode, encjson_key_dir)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        ON CONFLICT (deploy_target_id, environment_id)
        DO UPDATE SET
            env_repo_id = EXCLUDED.env_repo_id,
            env_repo_path = EXCLUDED.env_repo_path,
            env_repo_branch = EXCLUDED.env_repo_branch,
            deploy_repo_id = EXCLUDED.deploy_repo_id,
            deploy_repo_path = EXCLUDED.deploy_repo_path,
            deploy_repo_branch = EXCLUDED.deploy_repo_branch,
            allow_auto_release = EXCLUDED.allow_auto_release,
            append_env_suffix = EXCLUDED.append_env_suffix,
            is_active = EXCLUDED.is_active,
            release_manifest_mode = EXCLUDED.release_manifest_mode,
            encjson_key_dir = EXCLUDED.encjson_key_dir
        RETURNING *
        "#
    )
    .bind(deploy_target_id)
    .bind(environment.id)
    .bind(payload_env_repo_id)
    .bind(env_repo_path)
    .bind(env_repo_branch)
    .bind(payload_deploy_repo_id)
    .bind(deploy_repo_path)
    .bind(deploy_repo_branch)
    .bind(allow_auto_release)
    .bind(append_env_suffix)
    .bind(is_active)
    .bind(manifest_mode)
    .bind(encjson_key_dir)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to upsert deploy target env: {}", e),
            }),
        )
    })?;

    Ok(row)
}

#[derive(Clone)]
pub struct DeployApiState {
    pub pool: PgPool,
    pub encryption_secret: String,
    pub kube_build_app_path: String,
    pub apply_env_path: String,
    pub encjson_path: String,
    pub encjson_legacy_path: String,
    pub kubeconform_path: String,
    pub job_logs: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeployTargetRequest {
    pub name: String,
    pub envs: Option<Vec<DeployTargetEnvInput>>,
    pub env_name: Option<String>,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub encjson_private_key: Option<String>,
    pub encjson_keys: Option<Vec<EncjsonKeyInput>>,
    pub allow_auto_release: Option<bool>,
    pub append_env_suffix: Option<bool>,
    pub release_manifest_mode: Option<String>,
    pub is_active: Option<bool>,
    pub copy_from_target_id: Option<Uuid>,
    pub env_vars: Option<Vec<DeployTargetEnvVarInput>>,
    pub extra_env_vars: Option<Vec<DeployTargetExtraEnvVarInput>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeployTargetRequest {
    pub name: String,
    pub envs: Option<Vec<DeployTargetEnvInput>>,
    pub env_name: Option<String>,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub encjson_private_key: Option<String>,
    pub encjson_keys: Option<Vec<EncjsonKeyInput>>,
    pub allow_auto_release: Option<bool>,
    pub append_env_suffix: Option<bool>,
    pub release_manifest_mode: Option<String>,
    pub is_active: Option<bool>,
    pub is_archived: Option<bool>,
    pub env_vars: Option<Vec<DeployTargetEnvVarInput>>,
    pub extra_env_vars: Option<Vec<DeployTargetExtraEnvVarInput>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeployJobRequest {
    pub release_id: Uuid,
    pub environment_id: Uuid,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AutoDeployFromCopyJobRequest {
    pub copy_job_id: Uuid,
    pub environment_id: Uuid,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EnvironmentRequest {
    pub name: String,
    pub slug: Option<String>,
    pub color: Option<String>,
    pub source_registry_id: Option<Uuid>,
    pub target_registry_id: Option<Uuid>,
    pub source_project_path: Option<String>,
    pub target_project_path: Option<String>,
    pub source_auth_type: Option<String>,
    pub source_username: Option<String>,
    pub source_password: Option<String>,
    pub source_token: Option<String>,
    pub target_auth_type: Option<String>,
    pub target_username: Option<String>,
    pub target_password: Option<String>,
    pub target_token: Option<String>,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub allow_auto_release: Option<bool>,
    pub append_env_suffix: Option<bool>,
    pub release_manifest_mode: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub release_env_var_mappings: Option<Vec<DeployTargetEnvVarInput>>,
    pub extra_env_vars: Option<Vec<DeployTargetExtraEnvVarInput>>,
    pub argocd_poll_interval_seconds: Option<i32>,
    pub kubernetes_poll_interval_seconds: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployTargetEnvInput {
    pub environment_id: Uuid,
    pub env_repo_id: Uuid,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Uuid,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub allow_auto_release: Option<bool>,
    pub append_env_suffix: Option<bool>,
    pub release_manifest_mode: Option<String>,
    pub is_active: Option<bool>,
    pub encjson_key_dir: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EncjsonKeyInput {
    pub public_key: String,
    pub private_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployTargetEnvVarInput {
    pub source_key: String,
    pub target_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployTargetExtraEnvVarInput {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct DeployTargetWithKeys {
    pub target: DeployTargetSummary,
    pub encjson_keys: Vec<EncjsonKeySummary>,
    pub env_vars: Vec<DeployTargetEnvVar>,
    pub extra_env_vars: Vec<DeployTargetExtraEnvVar>,
}

#[derive(Debug, Serialize)]
pub struct DeployTargetSummary {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub is_archived: bool,
    pub has_jobs: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub envs: Vec<DeployTargetEnvSummary>,
}

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct DeployTargetEnvSummary {
    pub id: Uuid,
    pub deploy_target_id: Uuid,
    pub environment_id: Uuid,
    pub env_name: String,
    pub env_slug: String,
    pub env_color: Option<String>,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub allow_auto_release: bool,
    pub append_env_suffix: bool,
    pub release_manifest_mode: String,
    pub is_active: bool,
}

async fn get_deploy_target_summary(
    pool: &PgPool,
    target_id: Uuid,
) -> Result<DeployTargetSummary, (StatusCode, Json<ErrorResponse>)> {
    let base = sqlx::query_as::<_, (Uuid, Uuid, String, bool, bool, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT
            dt.id,
            dt.tenant_id,
            dt.name,
            dt.is_archived,
            EXISTS(SELECT 1 FROM deploy_jobs dj WHERE dj.deploy_target_id = dt.id) AS has_jobs,
            dt.created_at
        FROM deploy_targets dt
        WHERE dt.id = $1
        "#,
    )
    .bind(target_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let Some((id, tenant_id, name, is_archived, has_jobs, created_at)) = base else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Deploy target with id {} not found", target_id),
            }),
        ));
    };

    let envs = sqlx::query_as::<_, DeployTargetEnvSummary>(
        r#"
        SELECT
            dte.id,
            dte.deploy_target_id,
            dte.environment_id,
            e.name AS env_name,
            e.slug AS env_slug,
            e.color AS env_color,
            dte.env_repo_id,
            dte.env_repo_path,
            dte.env_repo_branch,
            dte.deploy_repo_id,
            dte.deploy_repo_path,
            dte.deploy_repo_branch,
            dte.encjson_key_dir,
            dte.allow_auto_release,
            dte.append_env_suffix,
            dte.release_manifest_mode,
            dte.is_active
        FROM deploy_target_envs dte
        JOIN environments e ON e.id = dte.environment_id
        WHERE dte.deploy_target_id = $1
        ORDER BY e.slug ASC
        "#
    )
    .bind(id)
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

    Ok(DeployTargetSummary {
        id,
        tenant_id,
        name,
        is_archived,
        has_jobs,
        created_at,
        envs,
    })
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
    pub environment_id: Uuid,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
    pub commit_sha: Option<String>,
    pub tag_name: Option<String>,
    pub target_name: String,
    pub env_name: String,
    pub env_color: Option<String>,
    pub is_auto: bool,
    pub copy_job_id: Option<Uuid>,
    pub bundle_id: Option<Uuid>,
    pub dry_run: bool,
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
    pub env_color: Option<String>,
    pub environment_id: Uuid,
    pub release_db_id: Uuid,
    pub release_id: String,
    pub is_auto: bool,
    pub bundle_id: Uuid,
    pub bundle_name: String,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub dry_run: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeployJobImageRow {
    pub file_path: String,
    pub container_name: String,
    pub image: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeployTargetEnvOption {
    pub deploy_target_id: Uuid,
    pub deploy_target_env_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub environment_id: Uuid,
    pub env_name: String,
    pub env_slug: String,
    pub env_color: Option<String>,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub allow_auto_release: bool,
    pub append_env_suffix: bool,
    pub release_manifest_mode: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn router(state: DeployApiState) -> Router {
    Router::new()
        .route("/tenants/{tenant_id}/environments", get(list_environments).post(create_environment))
        .route("/environments/{id}", get(get_environment).put(update_environment).delete(delete_environment))
        .route("/releases/{id}/deploy-jobs", get(list_release_deploy_jobs))
        .route("/deploy/jobs", get(list_deploy_jobs).post(create_deploy_job))
        .route("/deploy/jobs/from-copy", post(auto_deploy_from_copy_job))
        .route("/deploy/jobs/{id}", get(get_deploy_job))
        .route("/deploy/jobs/{id}/start", post(start_deploy_job))
        .route("/deploy/jobs/{id}/logs", get(deploy_job_logs_sse))
        .route("/deploy/jobs/{id}/logs/history", get(deploy_job_logs_history))
        .route("/deploy/jobs/{id}/diff", get(deploy_job_diff))
        .route("/deploy/jobs/{id}/images", get(deploy_job_images))
        .with_state(state)
}

async fn list_deploy_targets(
    State(state): State<DeployApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<DeployTargetSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let base_targets = sqlx::query_as::<_, (Uuid, Uuid, String, bool, bool, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT
            dt.id,
            dt.tenant_id,
            dt.name,
            dt.is_archived,
            EXISTS(SELECT 1 FROM deploy_jobs dj WHERE dj.deploy_target_id = dt.id) AS has_jobs,
            dt.created_at
        FROM deploy_targets dt
        WHERE dt.tenant_id = $1
        ORDER BY dt.created_at DESC
        "#
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

    let target_ids: Vec<Uuid> = base_targets.iter().map(|row| row.0).collect();
    let envs = if target_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, DeployTargetEnvSummary>(
            r#"
            SELECT
                dte.id,
                dte.deploy_target_id,
                dte.environment_id,
                e.name AS env_name,
                e.slug AS env_slug,
                e.color AS env_color,
                dte.env_repo_id,
                dte.env_repo_path,
                dte.env_repo_branch,
                dte.deploy_repo_id,
                dte.deploy_repo_path,
                dte.deploy_repo_branch,
                dte.encjson_key_dir,
                dte.allow_auto_release,
                dte.append_env_suffix,
                dte.release_manifest_mode,
                dte.is_active
            FROM deploy_target_envs dte
            JOIN environments e ON e.id = dte.environment_id
            WHERE dte.deploy_target_id = ANY($1)
            ORDER BY e.slug ASC
            "#
        )
        .bind(&target_ids)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?
    };

    let mut envs_by_target: HashMap<Uuid, Vec<DeployTargetEnvSummary>> = HashMap::new();
    for env in envs {
        envs_by_target.entry(env.deploy_target_id).or_default().push(env);
    }

    let targets = base_targets
        .into_iter()
        .map(|(id, tenant_id, name, is_archived, has_jobs, created_at)| DeployTargetSummary {
            id,
            tenant_id,
            name,
            is_archived,
            has_jobs,
            created_at,
            envs: envs_by_target.remove(&id).unwrap_or_default(),
        })
        .collect();

    Ok(Json(targets))
}

async fn list_environments(
    State(state): State<DeployApiState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<Environment>>, (StatusCode, Json<ErrorResponse>)> {
    let envs = sqlx::query_as::<_, Environment>(
        "SELECT * FROM environments WHERE tenant_id = $1 ORDER BY name",
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

    Ok(Json(envs))
}

async fn create_environment(
    State(state): State<DeployApiState>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<EnvironmentRequest>,
) -> Result<(StatusCode, Json<Environment>), (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment name cannot be empty".to_string(),
            }),
        ));
    }
    let slug = payload
        .slug
        .as_deref()
        .map(slugify_env_name)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slugify_env_name(name));

    let env_repo_path = sanitize_path(payload.env_repo_path);
    let deploy_repo_path = sanitize_path(payload.deploy_repo_path);
    if env_repo_path.is_some() && payload.env_repo_branch.as_deref().unwrap_or("").trim().len() > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment repo path or branch must be set (not both)".to_string(),
            }),
        ));
    }
    if deploy_repo_path.is_some() && payload.deploy_repo_branch.as_deref().unwrap_or("").trim().len() > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy repo path or branch must be set (not both)".to_string(),
            }),
        ));
    }

    let source_auth_type = payload.source_auth_type.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let target_auth_type = payload.target_auth_type.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let source_password_encrypted = payload.source_password.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| crypto::encrypt(v, &state.encryption_secret))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt source password: {}", e),
                }),
            )
        })?;
    let source_token_encrypted = payload.source_token.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| crypto::encrypt(v, &state.encryption_secret))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt source token: {}", e),
                }),
            )
        })?;
    let target_password_encrypted = payload.target_password.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| crypto::encrypt(v, &state.encryption_secret))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt target password: {}", e),
                }),
            )
        })?;
    let target_token_encrypted = payload.target_token.as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| crypto::encrypt(v, &state.encryption_secret))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt target token: {}", e),
                }),
            )
        })?;

    let env = sqlx::query_as::<_, Environment>(
        r#"
        INSERT INTO environments (
            tenant_id, name, slug, color,
            source_registry_id, target_registry_id,
            source_project_path, target_project_path,
            source_auth_type, source_username, source_password_encrypted, source_token_encrypted,
            target_auth_type, target_username, target_password_encrypted, target_token_encrypted,
            env_repo_id, env_repo_path, env_repo_branch,
            deploy_repo_id, deploy_repo_path, deploy_repo_branch,
            allow_auto_release, append_env_suffix, release_manifest_mode, encjson_key_dir,
            release_env_var_mappings, extra_env_vars, argocd_poll_interval_seconds, kubernetes_poll_interval_seconds
        )
        VALUES (
            $1, $2, $3, $4,
            $5, $6,
            $7, $8,
            $9, $10, $11, $12,
            $13, $14, $15, $16,
            $17, $18, $19,
            $20, $21, $22,
            $23, $24, $25, $26,
            $27, $28, $29, $30
        )
        RETURNING *
        "#
    )
    .bind(tenant_id)
    .bind(name)
    .bind(slug)
    .bind(payload.color)
    .bind(payload.source_registry_id)
    .bind(payload.target_registry_id)
    .bind(sanitize_path(payload.source_project_path))
    .bind(sanitize_path(payload.target_project_path))
    .bind(source_auth_type)
    .bind(payload.source_username)
    .bind(source_password_encrypted)
    .bind(source_token_encrypted)
    .bind(target_auth_type)
    .bind(payload.target_username)
    .bind(target_password_encrypted)
    .bind(target_token_encrypted)
    .bind(payload.env_repo_id)
    .bind(env_repo_path)
    .bind(payload.env_repo_branch.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(payload.deploy_repo_id)
    .bind(deploy_repo_path)
    .bind(payload.deploy_repo_branch.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(payload.allow_auto_release.unwrap_or(false))
    .bind(payload.append_env_suffix.unwrap_or(false))
    .bind(payload.release_manifest_mode.clone())
    .bind(payload.encjson_key_dir.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(env_vars_to_json(payload.release_env_var_mappings.clone()))
    .bind(extra_env_vars_to_json(payload.extra_env_vars.clone()))
    .bind(payload.argocd_poll_interval_seconds.unwrap_or(0))
    .bind(payload.kubernetes_poll_interval_seconds.unwrap_or(0))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        let msg = format!("Database error: {}", e);
        let status = if msg.contains("idx_environments_tenant_slug") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (
            status,
            Json(ErrorResponse {
                error: msg,
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(env)))
}

async fn get_environment(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Environment>, (StatusCode, Json<ErrorResponse>)> {
    let env = sqlx::query_as::<_, Environment>(
        "SELECT * FROM environments WHERE id = $1",
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

    match env {
        Some(env) => Ok(Json(env)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Environment with id {} not found", id),
            }),
        )),
    }
}

async fn update_environment(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<EnvironmentRequest>,
) -> Result<Json<Environment>, (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment name cannot be empty".to_string(),
            }),
        ));
    }
    let slug = payload
        .slug
        .as_deref()
        .map(slugify_env_name)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slugify_env_name(name));

    let current = sqlx::query_as::<_, Environment>(
        "SELECT * FROM environments WHERE id = $1",
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

    let Some(current) = current else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Environment with id {} not found", id),
            }),
        ));
    };

    let env_repo_path = sanitize_path(payload.env_repo_path);
    let deploy_repo_path = sanitize_path(payload.deploy_repo_path);
    if env_repo_path.is_some() && payload.env_repo_branch.as_deref().unwrap_or("").trim().len() > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment repo path or branch must be set (not both)".to_string(),
            }),
        ));
    }
    if deploy_repo_path.is_some() && payload.deploy_repo_branch.as_deref().unwrap_or("").trim().len() > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy repo path or branch must be set (not both)".to_string(),
            }),
        ));
    }

    let source_auth_type = payload.source_auth_type.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let target_auth_type = payload.target_auth_type.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let source_password_encrypted = match payload.source_password.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() => Some(crypto::encrypt(v, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt source password: {}", e),
                }),
            )
        })?),
        _ => current.source_password_encrypted.clone(),
    };
    let source_token_encrypted = match payload.source_token.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() => Some(crypto::encrypt(v, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt source token: {}", e),
                }),
            )
        })?),
        _ => current.source_token_encrypted.clone(),
    };
    let target_password_encrypted = match payload.target_password.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() => Some(crypto::encrypt(v, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt target password: {}", e),
                }),
            )
        })?),
        _ => current.target_password_encrypted.clone(),
    };
    let target_token_encrypted = match payload.target_token.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() => Some(crypto::encrypt(v, &state.encryption_secret).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to encrypt target token: {}", e),
                }),
            )
        })?),
        _ => current.target_token_encrypted.clone(),
    };

    let env = sqlx::query_as::<_, Environment>(
        r#"
        UPDATE environments
        SET name = $1,
            slug = $2,
            color = $3,
            source_registry_id = $4,
            target_registry_id = $5,
            source_project_path = $6,
            target_project_path = $7,
            source_auth_type = $8,
            source_username = $9,
            source_password_encrypted = $10,
            source_token_encrypted = $11,
            target_auth_type = $12,
            target_username = $13,
            target_password_encrypted = $14,
            target_token_encrypted = $15,
            env_repo_id = $16,
            env_repo_path = $17,
            env_repo_branch = $18,
            deploy_repo_id = $19,
            deploy_repo_path = $20,
            deploy_repo_branch = $21,
            allow_auto_release = $22,
            append_env_suffix = $23,
            release_manifest_mode = $24,
            encjson_key_dir = $25,
            release_env_var_mappings = $26,
            extra_env_vars = $27,
            argocd_poll_interval_seconds = $28,
            kubernetes_poll_interval_seconds = $29
        WHERE id = $30
        RETURNING *
        "#
    )
    .bind(name)
    .bind(slug)
    .bind(payload.color)
    .bind(payload.source_registry_id)
    .bind(payload.target_registry_id)
    .bind(sanitize_path(payload.source_project_path))
    .bind(sanitize_path(payload.target_project_path))
    .bind(source_auth_type)
    .bind(payload.source_username)
    .bind(source_password_encrypted)
    .bind(source_token_encrypted)
    .bind(target_auth_type)
    .bind(payload.target_username)
    .bind(target_password_encrypted)
    .bind(target_token_encrypted)
    .bind(payload.env_repo_id)
    .bind(env_repo_path)
    .bind(payload.env_repo_branch.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(payload.deploy_repo_id)
    .bind(deploy_repo_path)
    .bind(payload.deploy_repo_branch.as_deref().map(str::trim).filter(|v| !v.is_empty()))
    .bind(payload.allow_auto_release.unwrap_or(current.allow_auto_release))
    .bind(payload.append_env_suffix.unwrap_or(current.append_env_suffix))
    .bind(payload.release_manifest_mode.clone().or_else(|| current.release_manifest_mode.clone()))
    .bind(payload.encjson_key_dir.as_deref().map(str::trim).filter(|v| !v.is_empty()).or_else(|| current.encjson_key_dir.as_deref()))
    .bind(if payload.release_env_var_mappings.is_some() { env_vars_to_json(payload.release_env_var_mappings.clone()) } else { current.release_env_var_mappings.clone() })
    .bind(if payload.extra_env_vars.is_some() { extra_env_vars_to_json(payload.extra_env_vars.clone()) } else { current.extra_env_vars.clone() })
    .bind(payload.argocd_poll_interval_seconds.unwrap_or(current.argocd_poll_interval_seconds))
    .bind(payload.kubernetes_poll_interval_seconds.unwrap_or(current.kubernetes_poll_interval_seconds))
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

    match env {
        Some(env) => Ok(Json(env)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Environment with id {} not found", id),
            }),
        )),
    }
}

async fn delete_environment(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let in_use = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM deploy_jobs WHERE environment_id = $1)",
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

    if in_use {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is used by deploy jobs and cannot be deleted".to_string(),
            }),
        ));
    }

    let in_use_copy = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM copy_jobs WHERE environment_id = $1)",
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

    if in_use_copy {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is used by copy jobs and cannot be deleted".to_string(),
            }),
        ));
    }

    let result = sqlx::query("DELETE FROM environments WHERE id = $1")
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
                error: format!("Environment with id {} not found", id),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeployTargetWithKeys>, (StatusCode, Json<ErrorResponse>)> {
    let target = sqlx::query_as::<_, (Uuid, Uuid, String, bool, bool, chrono::DateTime<chrono::Utc>)>(
        r#"
        SELECT
            dt.id,
            dt.tenant_id,
            dt.name,
            dt.is_archived,
            EXISTS(SELECT 1 FROM deploy_jobs dj WHERE dj.deploy_target_id = dt.id) AS has_jobs,
            dt.created_at
        FROM deploy_targets dt
        WHERE dt.id = $1
        "#
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

    match target {
        Some((id, tenant_id, name, is_archived, has_jobs, created_at)) => {
            let envs = sqlx::query_as::<_, DeployTargetEnvSummary>(
                r#"
                SELECT
                    dte.id,
                    dte.deploy_target_id,
                    dte.environment_id,
                    e.name AS env_name,
                    e.slug AS env_slug,
                    e.color AS env_color,
                    dte.env_repo_id,
                    dte.env_repo_path,
                    dte.env_repo_branch,
                    dte.deploy_repo_id,
                    dte.deploy_repo_path,
                    dte.deploy_repo_branch,
                    dte.encjson_key_dir,
                    dte.allow_auto_release,
                    dte.append_env_suffix,
                    dte.release_manifest_mode,
                    dte.is_active
                FROM deploy_target_envs dte
                JOIN environments e ON e.id = dte.environment_id
                WHERE dte.deploy_target_id = $1
                ORDER BY e.slug ASC
                "#
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

            let summary = DeployTargetSummary {
                id,
                tenant_id,
                name,
                is_archived,
                has_jobs,
                created_at,
                envs,
            };

            let keys = sqlx::query_as::<_, DeployTargetEncjsonKey>(
                "SELECT * FROM deploy_target_encjson_keys WHERE deploy_target_id = $1 ORDER BY created_at",
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

            let env_vars = sqlx::query_as::<_, DeployTargetEnvVar>(
                "SELECT * FROM deploy_target_env_vars WHERE deploy_target_id = $1 ORDER BY target_key",
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

            let summaries = keys
                .into_iter()
                .map(|k| EncjsonKeySummary {
                    public_key: k.public_key,
                    has_private: true,
                })
                .collect();

            let extra_env_vars = sqlx::query_as::<_, DeployTargetExtraEnvVar>(
                "SELECT * FROM deploy_target_extra_env_vars WHERE deploy_target_id = $1 ORDER BY key",
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

            Ok(Json(DeployTargetWithKeys {
                target: summary,
                encjson_keys: summaries,
                env_vars,
                extra_env_vars,
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
) -> Result<(StatusCode, Json<DeployTargetSummary>), (StatusCode, Json<ErrorResponse>)> {
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name is required".to_string(),
            }),
        ));
    }
    let use_envs = payload.envs.as_ref().map(|v| !v.is_empty()).unwrap_or(false);

    let payload_env_name = payload.env_name.clone().unwrap_or_default();
    if !use_envs && payload_env_name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment name is required".to_string(),
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

    let mut env_entries: Vec<(Environment, DeployTargetEnvInput)> = Vec::new();
    let mut base_env_name = payload_env_name.clone();
    let mut base_env_repo_id = payload.env_repo_id.unwrap_or_else(Uuid::nil);
    let mut base_env_repo_path = payload
        .env_repo_path
        .clone()
        .unwrap_or_else(|| payload_env_name.clone());
    let mut base_deploy_repo_id = payload.deploy_repo_id.unwrap_or_else(Uuid::nil);
    let mut base_deploy_repo_path = payload
        .deploy_repo_path
        .clone()
        .unwrap_or_else(|| format!("deploy/{}", payload_env_name));
    let mut base_allow_auto_release = payload.allow_auto_release.unwrap_or(false);
    let mut base_append_env_suffix = payload.append_env_suffix.unwrap_or(false);
    let mut base_release_manifest_mode = payload
        .release_manifest_mode
        .clone()
        .unwrap_or_else(|| "match_digest".to_string());
    let mut base_is_active = payload.is_active.unwrap_or(true);
    let mut base_encjson_key_dir = payload.encjson_key_dir.clone();

    if use_envs {
        for entry in payload.envs.clone().unwrap_or_default() {
            let environment = sqlx::query_as::<_, Environment>(
                "SELECT * FROM environments WHERE id = $1 AND tenant_id = $2",
            )
            .bind(entry.environment_id)
            .bind(tenant_id)
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
                        error: "Environment not found for tenant".to_string(),
                    }),
                )
            })?;

            let env_repo_ok = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
            )
            .bind(entry.env_repo_id)
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
            .bind(entry.deploy_repo_id)
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

            let env_path_set = entry.env_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            let env_branch_set = entry.env_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            if env_path_set == env_branch_set {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Environment repo path or branch must be set (not both)".to_string(),
                    }),
                ));
            }

            let deploy_path_set = entry.deploy_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            let deploy_branch_set = entry.deploy_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            if deploy_path_set == deploy_branch_set {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Deploy repo path or branch must be set (not both)".to_string(),
                    }),
                ));
            }

            env_entries.push((environment, entry));
        }

        if env_entries.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "At least one environment is required".to_string(),
                }),
            ));
        }

        if let Some((environment, first_env)) = env_entries.first() {
            base_env_name = environment.slug.clone();
            base_env_repo_id = first_env.env_repo_id;
            base_env_repo_path = first_env
                .env_repo_path
                .clone()
                .unwrap_or_else(|| environment.slug.clone());
            base_deploy_repo_id = first_env.deploy_repo_id;
            base_deploy_repo_path = first_env
                .deploy_repo_path
                .clone()
                .unwrap_or_else(|| format!("deploy/{}", environment.slug));
            base_allow_auto_release = first_env.allow_auto_release.unwrap_or(false);
            base_append_env_suffix = first_env.append_env_suffix.unwrap_or(false);
            base_release_manifest_mode = first_env
                .release_manifest_mode
                .clone()
                .unwrap_or_else(|| "match_digest".to_string());
            base_is_active = first_env.is_active.unwrap_or(true);
            if first_env.encjson_key_dir.is_some() {
                base_encjson_key_dir = first_env.encjson_key_dir.clone();
            }
        }
    } else {
        let env_repo_id = payload.env_repo_id.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment repository is required".to_string(),
                }),
            )
        })?;

        let deploy_repo_id = payload.deploy_repo_id.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Deploy repository is required".to_string(),
                }),
            )
        })?;

        base_env_repo_id = env_repo_id;
        base_deploy_repo_id = deploy_repo_id;

        let env_repo_ok = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
        )
        .bind(env_repo_id)
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
        .bind(deploy_repo_id)
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

        let env_path_set = payload.env_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        let env_branch_set = payload.env_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        if env_path_set == env_branch_set {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment repo path or branch must be set (not both)".to_string(),
                }),
            ));
        }

        let deploy_path_set = payload.deploy_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        let deploy_branch_set = payload.deploy_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        if deploy_path_set == deploy_branch_set {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Deploy repo path or branch must be set (not both)".to_string(),
                }),
            ));
        }

        let environment = ensure_environment(&state.pool, tenant_id, &payload_env_name).await?;
        let env_input = DeployTargetEnvInput {
            environment_id: environment.id,
            env_repo_id,
            env_repo_path: payload.env_repo_path.clone(),
            env_repo_branch: payload.env_repo_branch.clone(),
            deploy_repo_id,
            deploy_repo_path: payload.deploy_repo_path.clone(),
            deploy_repo_branch: payload.deploy_repo_branch.clone(),
            allow_auto_release: payload.allow_auto_release,
            append_env_suffix: payload.append_env_suffix,
            release_manifest_mode: payload.release_manifest_mode.clone(),
            is_active: payload.is_active,
            encjson_key_dir: payload.encjson_key_dir.clone(),
        };
        env_entries.push((environment, env_input));
    }

    let target = sqlx::query_as::<_, DeployTarget>(
        r#"
        INSERT INTO deploy_targets
        (tenant_id, name, env_name, env_repo_id, env_repo_path,
         deploy_repo_id, deploy_repo_path, deploy_path, encjson_key_dir, encjson_private_key_encrypted,
         allow_auto_release, append_env_suffix, release_manifest_mode, is_active)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING *
        "#,
    )
    .bind(tenant_id)
    .bind(&payload.name)
    .bind(&base_env_name)
    .bind(base_env_repo_id)
    .bind(&base_env_repo_path)
    .bind(base_deploy_repo_id)
    .bind(&base_deploy_repo_path)
    .bind(&base_deploy_repo_path)
    .bind(&base_encjson_key_dir)
    .bind(&encjson_private_key_encrypted)
    .bind(base_allow_auto_release)
    .bind(base_append_env_suffix)
    .bind(base_release_manifest_mode)
    .bind(base_is_active)
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

    for (environment, entry) in &env_entries {
        let _ = upsert_deploy_target_env(
            &state.pool,
            target.id,
            environment,
            entry.env_repo_id,
            entry.env_repo_path.clone(),
            entry.env_repo_branch.clone(),
            entry.deploy_repo_id,
            entry.deploy_repo_path.clone(),
            entry.deploy_repo_branch.clone(),
            entry.allow_auto_release.unwrap_or(false),
            entry.append_env_suffix.unwrap_or(false),
            entry.is_active.unwrap_or(true),
            entry.release_manifest_mode.clone(),
            entry.encjson_key_dir.clone(),
        )
        .await?;
    }

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

    if let Some(env_vars) = payload.env_vars {
        store_deploy_target_env_vars(&state, target.id, env_vars).await?;
    } else if let Some(source_id) = payload.copy_from_target_id {
        sqlx::query(
            r#"
            INSERT INTO deploy_target_env_vars (deploy_target_id, source_key, target_key)
            SELECT $1, source_key, target_key
            FROM deploy_target_env_vars
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

    if let Some(extra_env_vars) = payload.extra_env_vars {
        store_deploy_target_extra_env_vars(&state, target.id, extra_env_vars).await?;
    } else if let Some(source_id) = payload.copy_from_target_id {
        sqlx::query(
            r#"
            INSERT INTO deploy_target_extra_env_vars (deploy_target_id, key, value)
            SELECT $1, key, value
            FROM deploy_target_extra_env_vars
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

    let summary = get_deploy_target_summary(&state.pool, target.id).await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn update_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateDeployTargetRequest>,
) -> Result<Json<DeployTargetSummary>, (StatusCode, Json<ErrorResponse>)> {
    let use_envs = payload.envs.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Name is required".to_string(),
            }),
        ));
    }
    let payload_env_name = payload.env_name.clone().unwrap_or_default();
    if !use_envs && payload_env_name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment name is required".to_string(),
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

    let mut env_entries: Vec<(Environment, DeployTargetEnvInput)> = Vec::new();
    let mut base_env_name = payload_env_name.clone();
    let mut base_env_repo_id = payload.env_repo_id.unwrap_or_else(Uuid::nil);
    let mut base_env_repo_path = payload
        .env_repo_path
        .clone()
        .unwrap_or_else(|| payload_env_name.clone());
    let mut base_deploy_repo_id = payload.deploy_repo_id.unwrap_or_else(Uuid::nil);
    let mut base_deploy_repo_path = payload
        .deploy_repo_path
        .clone()
        .unwrap_or_else(|| format!("deploy/{}", payload_env_name));
    let mut base_allow_auto_release = payload.allow_auto_release.unwrap_or(false);
    let mut base_append_env_suffix = payload.append_env_suffix.unwrap_or(false);
    let mut base_release_manifest_mode = payload
        .release_manifest_mode
        .clone()
        .unwrap_or_else(|| "match_digest".to_string());
    let mut base_is_active = payload.is_active.unwrap_or(true);
    let mut base_encjson_key_dir = payload.encjson_key_dir.clone();

    if use_envs {
        for entry in payload.envs.clone().unwrap_or_default() {
            let environment = sqlx::query_as::<_, Environment>(
                "SELECT * FROM environments WHERE id = $1 AND tenant_id = $2",
            )
            .bind(entry.environment_id)
            .bind(target_tenant)
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
                        error: "Environment not found for tenant".to_string(),
                    }),
                )
            })?;

            let env_repo_ok = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
            )
            .bind(entry.env_repo_id)
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
            .bind(entry.deploy_repo_id)
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

            let env_path_set = entry.env_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            let env_branch_set = entry.env_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            if env_path_set == env_branch_set {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Environment repo path or branch must be set (not both)".to_string(),
                    }),
                ));
            }

            let deploy_path_set = entry.deploy_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            let deploy_branch_set = entry.deploy_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
            if deploy_path_set == deploy_branch_set {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Deploy repo path or branch must be set (not both)".to_string(),
                    }),
                ));
            }

            env_entries.push((environment, entry));
        }

        if env_entries.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "At least one environment is required".to_string(),
                }),
            ));
        }

        if let Some((environment, first_env)) = env_entries.first() {
            base_env_name = environment.slug.clone();
            base_env_repo_id = first_env.env_repo_id;
            base_env_repo_path = first_env
                .env_repo_path
                .clone()
                .unwrap_or_else(|| environment.slug.clone());
            base_deploy_repo_id = first_env.deploy_repo_id;
            base_deploy_repo_path = first_env
                .deploy_repo_path
                .clone()
                .unwrap_or_else(|| format!("deploy/{}", environment.slug));
            base_allow_auto_release = first_env.allow_auto_release.unwrap_or(false);
            base_append_env_suffix = first_env.append_env_suffix.unwrap_or(false);
            base_release_manifest_mode = first_env
                .release_manifest_mode
                .clone()
                .unwrap_or_else(|| "match_digest".to_string());
            base_is_active = first_env.is_active.unwrap_or(true);
            if first_env.encjson_key_dir.is_some() {
                base_encjson_key_dir = first_env.encjson_key_dir.clone();
            }
        }
    } else {
        let env_repo_id = payload.env_repo_id.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment repository is required".to_string(),
                }),
            )
        })?;

        let deploy_repo_id = payload.deploy_repo_id.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Deploy repository is required".to_string(),
                }),
            )
        })?;

        base_env_repo_id = env_repo_id;
        base_deploy_repo_id = deploy_repo_id;

        let env_repo_ok = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM git_repositories WHERE id = $1 AND tenant_id = $2)",
        )
        .bind(env_repo_id)
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
        .bind(deploy_repo_id)
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

        let env_path_set = payload.env_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        let env_branch_set = payload.env_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        if env_path_set == env_branch_set {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment repo path or branch must be set (not both)".to_string(),
                }),
            ));
        }

        let deploy_path_set = payload.deploy_repo_path.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        let deploy_branch_set = payload.deploy_repo_branch.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false);
        if deploy_path_set == deploy_branch_set {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Deploy repo path or branch must be set (not both)".to_string(),
                }),
            ));
        }

        let environment = ensure_environment(&state.pool, target_tenant, &payload_env_name).await?;
        let env_input = DeployTargetEnvInput {
            environment_id: environment.id,
            env_repo_id,
            env_repo_path: payload.env_repo_path.clone(),
            env_repo_branch: payload.env_repo_branch.clone(),
            deploy_repo_id,
            deploy_repo_path: payload.deploy_repo_path.clone(),
            deploy_repo_branch: payload.deploy_repo_branch.clone(),
            allow_auto_release: payload.allow_auto_release,
            append_env_suffix: payload.append_env_suffix,
            release_manifest_mode: payload.release_manifest_mode.clone(),
            is_active: payload.is_active,
            encjson_key_dir: payload.encjson_key_dir.clone(),
        };
        env_entries.push((environment, env_input));
    }

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
            release_manifest_mode = $12,
            is_active = $13,
            is_archived = COALESCE($14, is_archived)
        WHERE id = $15
        RETURNING *
        "#,
    )
    .bind(&payload.name)
    .bind(&base_env_name)
    .bind(base_env_repo_id)
    .bind(&base_env_repo_path)
    .bind(base_deploy_repo_id)
    .bind(&base_deploy_repo_path)
    .bind(&base_deploy_repo_path)
    .bind(&base_encjson_key_dir)
    .bind(&encjson_private_key_encrypted)
    .bind(base_allow_auto_release)
    .bind(base_append_env_suffix)
    .bind(base_release_manifest_mode)
    .bind(base_is_active)
    .bind(payload.is_archived)
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
            for (environment, entry) in &env_entries {
                let _ = upsert_deploy_target_env(
                    &state.pool,
                    target.id,
                    environment,
                    entry.env_repo_id,
                    entry.env_repo_path.clone(),
                    entry.env_repo_branch.clone(),
                    entry.deploy_repo_id,
                    entry.deploy_repo_path.clone(),
                    entry.deploy_repo_branch.clone(),
                    entry.allow_auto_release.unwrap_or(false),
                    entry.append_env_suffix.unwrap_or(false),
                    entry.is_active.unwrap_or(true),
                    entry.release_manifest_mode.clone(),
                    entry.encjson_key_dir.clone(),
                )
                .await?;
            }

            if let Some(keys) = payload.encjson_keys {
                store_encjson_keys(&state, target.id, keys).await?;
            }
            if let Some(env_vars) = payload.env_vars {
                store_deploy_target_env_vars(&state, target.id, env_vars).await?;
            }
            if let Some(extra_env_vars) = payload.extra_env_vars {
                store_deploy_target_extra_env_vars(&state, target.id, extra_env_vars).await?;
            }
            let summary = get_deploy_target_summary(&state.pool, target.id).await?;
            Ok(Json(summary))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Deploy target with id {} not found", id),
            }),
        )),
    }
}

#[derive(Debug, Serialize)]
struct DeployTargetDeleteResponse {
    archived: bool,
    message: String,
}

async fn delete_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<DeployTargetDeleteResponse>), (StatusCode, Json<ErrorResponse>)> {
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
        let result = sqlx::query("UPDATE deploy_targets SET is_archived = TRUE WHERE id = $1")
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

        return Ok((
            StatusCode::OK,
            Json(DeployTargetDeleteResponse {
                archived: true,
                message: "Deploy target archived".to_string(),
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

    Ok((
        StatusCode::OK,
        Json(DeployTargetDeleteResponse {
            archived: false,
            message: "Deploy target deleted".to_string(),
        }),
    ))
}

async fn set_deploy_target_archived(
    state: &DeployApiState,
    id: Uuid,
    archived: bool,
) -> Result<(StatusCode, Json<DeployTargetDeleteResponse>), (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("UPDATE deploy_targets SET is_archived = $1 WHERE id = $2")
        .bind(archived)
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

    Ok((
        StatusCode::OK,
        Json(DeployTargetDeleteResponse {
            archived,
            message: if archived {
                "Deploy target archived".to_string()
            } else {
                "Deploy target unarchived".to_string()
            },
        }),
    ))
}

async fn archive_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<DeployTargetDeleteResponse>), (StatusCode, Json<ErrorResponse>)> {
    set_deploy_target_archived(&state, id, true).await
}

async fn unarchive_deploy_target(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<DeployTargetDeleteResponse>), (StatusCode, Json<ErrorResponse>)> {
    set_deploy_target_archived(&state, id, false).await
}

async fn list_release_deploy_targets(
    State(state): State<DeployApiState>,
    Path(release_id): Path<Uuid>,
) -> Result<Json<Vec<DeployTargetEnvOption>>, (StatusCode, Json<ErrorResponse>)> {
    let targets = sqlx::query_as::<_, DeployTargetEnvOption>(
        r#"
        SELECT
            dt.id AS deploy_target_id,
            dte.id AS deploy_target_env_id,
            dt.tenant_id,
            dt.name,
            dte.environment_id,
            e.name AS env_name,
            e.slug AS env_slug,
            e.color AS env_color,
            dte.env_repo_id,
            dte.env_repo_path,
            dte.env_repo_branch,
            dte.deploy_repo_id,
            dte.deploy_repo_path,
            dte.deploy_repo_branch,
            dte.allow_auto_release,
            dte.append_env_suffix,
            dte.release_manifest_mode,
            dte.is_active
        FROM deploy_targets dt
        JOIN deploy_target_envs dte ON dte.deploy_target_id = dt.id
        JOIN environments e ON e.id = dte.environment_id
        JOIN releases r ON r.id = $1
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        WHERE dt.tenant_id = b.tenant_id
          AND dt.is_active = TRUE
          AND dt.is_archived = FALSE
        ORDER BY dt.created_at DESC, e.slug ASC
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
        SELECT dj.id, dj.release_id, dj.environment_id, dj.status, dj.started_at, dj.completed_at,
               dj.error_message, dj.commit_sha, dj.tag_name,
               e.name as target_name, e.slug AS env_name, e.color AS env_color,
               r.is_auto, r.copy_job_id, b.id as bundle_id, dj.dry_run
        FROM deploy_jobs dj
        JOIN environments e ON e.id = dj.environment_id
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
            e.name as target_name,
            e.slug AS env_name,
            e.color AS env_color,
            dj.environment_id,
            r.id as release_db_id,
            r.release_id,
            r.is_auto,
            b.id as bundle_id,
            b.name as bundle_name,
            t.id as tenant_id,
            t.name as tenant_name,
            dj.dry_run
        FROM deploy_jobs dj
        JOIN environments e ON e.id = dj.environment_id
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
        SELECT dj.id, dj.release_id, dj.environment_id, dj.status, dj.started_at, dj.completed_at,
               dj.error_message, dj.commit_sha, dj.tag_name,
               e.name as target_name, e.slug AS env_name, e.color AS env_color,
               r.is_auto, r.copy_job_id, b.id as bundle_id, dj.dry_run
        FROM deploy_jobs dj
        JOIN environments e ON e.id = dj.environment_id
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

    let environment = sqlx::query_as::<_, Environment>(
        "SELECT * FROM environments WHERE id = $1",
    )
    .bind(payload.environment_id)
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
                error: "Environment not found".to_string(),
            }),
        )
    })?;

    let release_tenant_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT b.tenant_id
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        WHERE r.id = $1
        "#,
    )
    .bind(payload.release_id)
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
                error: "Release tenant not found".to_string(),
            }),
        )
    })?;

    if environment.tenant_id != release_tenant_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment does not belong to this tenant".to_string(),
            }),
        ));
    }

    let job_id = create_deploy_job_record(
        &state,
        payload.release_id,
        environment.id,
        payload.dry_run.unwrap_or(true),
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(DeployJobResponse {
            job_id,
            message: "Deploy job created".to_string(),
        }),
    ))
}

async fn auto_deploy_from_copy_job(
    State(state): State<DeployApiState>,
    Json(payload): Json<AutoDeployFromCopyJobRequest>,
) -> Result<(StatusCode, Json<DeployJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let environment = sqlx::query_as::<_, Environment>(
        "SELECT * FROM environments WHERE id = $1",
    )
    .bind(payload.environment_id)
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
                error: "Environment not found".to_string(),
            }),
        )
    })?;

    if !environment.allow_auto_release {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment does not allow auto release".to_string(),
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

    if tenant_id != environment.tenant_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment does not belong to this tenant".to_string(),
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
                "INSERT INTO releases (copy_job_id, release_id, status, source_ref_mode, notes, created_by, is_auto, auto_reason)
                 VALUES ($1, $2, 'draft', 'tag', $3, $4, true, $5)
                 RETURNING id, copy_job_id, release_id, status, source_ref_mode, notes, created_by, is_auto, auto_reason, created_at",
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

    let dry_run = payload.dry_run.unwrap_or(true);
    let job_id = create_deploy_job_record(&state, release.id, environment.id, dry_run).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(DeployJobResponse {
            job_id,
            message: "Deploy job created".to_string(),
        }),
    ))
}

async fn create_deploy_job_record(
    state: &DeployApiState,
    release_id: Uuid,
    environment_id: Uuid,
    dry_run: bool,
) -> Result<Uuid, (StatusCode, Json<ErrorResponse>)> {
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO deploy_jobs (id, release_id, environment_id, status, dry_run)
         VALUES ($1, $2, $3, 'pending', $4)",
    )
    .bind(job_id)
    .bind(release_id)
    .bind(environment_id)
    .bind(dry_run)
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

    ensure_deploy_job_log_channel(state, job_id).await;

    Ok(job_id)
}

async fn ensure_deploy_job_log_channel(state: &DeployApiState, job_id: Uuid) -> broadcast::Sender<String> {
    let mut logs = state.job_logs.write().await;
    if let Some(existing) = logs.get(&job_id) {
        return existing.clone();
    }
    let (log_tx, _log_rx) = broadcast::channel(512);
    logs.insert(job_id, log_tx.clone());

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

    log_tx
}

async fn start_deploy_job(
    State(state): State<DeployApiState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<DeployJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let updated = sqlx::query(
        "UPDATE deploy_jobs SET status = 'in_progress', started_at = NOW() WHERE id = $1 AND status = 'pending'",
    )
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

    if updated.rows_affected() == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Deploy job is not pending".to_string(),
            }),
        ));
    }

    let log_tx = ensure_deploy_job_log_channel(&state, id).await;
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = run_deploy_job(state_clone.clone(), id, log_tx.clone()).await {
            let _ = log_tx.send(format!("Deploy job failed: {}", e));
            let _ = sqlx::query(
                "UPDATE deploy_jobs SET status = 'failed', completed_at = NOW(), error_message = $1 WHERE id = $2",
            )
            .bind(e.to_string())
            .bind(id)
            .execute(&state_clone.pool)
            .await;
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(DeployJobResponse {
            job_id: id,
            message: "Deploy job started".to_string(),
        }),
    ))
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

async fn store_deploy_target_env_vars(
    state: &DeployApiState,
    deploy_target_id: Uuid,
    vars: Vec<DeployTargetEnvVarInput>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let mut cleaned = Vec::new();
    for item in vars {
        let source_key = item.source_key.trim().to_string();
        let target_key = item.target_key.trim().to_string();
        if source_key.is_empty() || target_key.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Env var mapping must include source and target keys".to_string(),
                }),
            ));
        }
        cleaned.push((source_key, target_key));
    }

    let mut tx = state.pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    sqlx::query("DELETE FROM deploy_target_env_vars WHERE deploy_target_id = $1")
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

    for (source_key, target_key) in cleaned {
        sqlx::query(
            "INSERT INTO deploy_target_env_vars (deploy_target_id, source_key, target_key) VALUES ($1, $2, $3)",
        )
        .bind(deploy_target_id)
        .bind(&source_key)
        .bind(&target_key)
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

async fn store_deploy_target_extra_env_vars(
    state: &DeployApiState,
    deploy_target_id: Uuid,
    vars: Vec<DeployTargetExtraEnvVarInput>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let mut cleaned = Vec::new();
    for item in vars {
        let key = item.key.trim().to_string();
        let value = item.value.trim().to_string();
        if key.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Extra env vars must include a key".to_string(),
                }),
            ));
        }
        cleaned.push((key, value));
    }

    let mut tx = state.pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    sqlx::query("DELETE FROM deploy_target_extra_env_vars WHERE deploy_target_id = $1")
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

    for (key, value) in cleaned {
        sqlx::query(
            "INSERT INTO deploy_target_extra_env_vars (deploy_target_id, key, value) VALUES ($1, $2, $3)",
        )
        .bind(deploy_target_id)
        .bind(&key)
        .bind(&value)
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

    let environment = sqlx::query_as::<_, Environment>(
        "SELECT * FROM environments WHERE id = $1",
    )
    .bind(job.environment_id)
    .fetch_one(&state.pool)
    .await?;

    let release = sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = $1")
        .bind(job.release_id)
        .fetch_one(&state.pool)
        .await?;

    let env_var_rows = env_vars_from_json(&environment.release_env_var_mappings);
    let extra_env_rows = extra_env_vars_from_json(&environment.extra_env_vars);
    let mapped_vars = build_release_env_var_map(&env_var_rows, &release, &log_tx);

    let temp_dir = TempDir::new()?;
    let env_repo_path = temp_dir.path().join("environments");
    let deploy_repo_path = temp_dir.path().join("deploy");

    let env_repo_id = environment
        .env_repo_id
        .ok_or_else(|| anyhow::anyhow!("Deploy target env missing env_repo_id"))?;
    let deploy_repo_id = environment
        .deploy_repo_id
        .ok_or_else(|| anyhow::anyhow!("Deploy target env missing deploy_repo_id"))?;

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

    let env_branch = environment
        .env_repo_branch
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(&env_repo.default_branch);
    let deploy_branch = environment
        .deploy_repo_branch
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(&deploy_repo.default_branch);

    run_git_clone(&env_repo.repo_url, env_branch, &env_repo_path, &git_env_env, &log_tx).await?;
    run_git_clone(&deploy_repo.repo_url, deploy_branch, &deploy_repo_path, &git_env_deploy, &log_tx).await?;

    let mut release_manifest = build_release_manifest(&state.pool, release.id).await?;
    let env_repo_subdir = environment
        .env_repo_path
        .as_deref()
        .unwrap_or(&environment.slug);
    let env_repo_subdir = env_repo_subdir.trim().trim_start_matches('/').to_string();
    apply_release_manifest_mode(
        environment
            .release_manifest_mode
            .as_deref()
            .unwrap_or("strict"),
        &mut release_manifest,
        &env_repo_path,
        &environment.slug,
        Some(env_repo_subdir.as_str()),
    )
    .await?;
    let manifest_path = temp_dir.path().join("release-manifest.yml");
    let yaml = serde_yaml_ng::to_string(&release_manifest)?;
    tokio::fs::write(&manifest_path, yaml)
        .await
        .with_context(|| format!("Failed to write release manifest to {}", manifest_path.display()))?;

    let deploy_rel_path = environment
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

    let kube_build_env = build_kube_build_env(
        &environment.slug,
        &release,
        &manifest_path,
        &env_repo_path,
        &mapped_vars,
        &extra_env_rows,
    )?;
    run_command_logged(
        &state.kube_build_app_path,
        &["-e", &environment.slug, "-t", deploy_path.to_string_lossy().as_ref(), "-r", manifest_path.to_string_lossy().as_ref()],
        Some(&env_repo_path),
        &kube_build_env,
        &log_tx,
        "kube_build_app",
    )
    .await?;

    run_command_logged(
        &state.kube_build_app_path,
        &["-e", &environment.slug, "-s"],
        Some(&env_repo_path),
        &kube_build_env,
        &log_tx,
        "kube_build_app -s",
    )
    .await?;

    let env_file_path = temp_dir.path().join("release.env");
    build_env_file(
        &state,
        &environment,
        environment.encjson_key_dir.as_deref(),
        &env_repo_path,
        env_repo_subdir.as_str(),
        &env_file_path,
        &release,
        &env_var_rows,
        &extra_env_rows,
        &log_tx,
    )
    .await?;

    apply_env_to_outputs(&state, &deploy_path, &env_file_path, &log_tx).await?;

    if let Err(err) = collect_and_store_deploy_images(&state.pool, job_id, &deploy_path, &log_tx).await {
        let _ = log_tx.send(format!("Failed to collect deploy images (ignored): {}", err));
    }

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
    let tag_name = if environment.append_env_suffix {
        format!("{}-{}", release.release_id, environment.slug)
    } else {
        release.release_id.clone()
    };

    if let Some(diff) = diff_info {
        let _ = sqlx::query(
            "INSERT INTO deploy_job_diffs (deploy_job_id, files_changed, diff_patch) VALUES ($1, $2, $3)",
        )
        .bind(job_id)
        .bind(diff.files_changed)
        .bind(diff.diff_patch)
        .execute(&state.pool)
        .await;

        if job.dry_run {
            let _ = log_tx.send("Dry run enabled: skipping git add/commit/push/tag".to_string());
        } else {
            run_git_commit_and_push(
                &deploy_repo_path,
                deploy_rel_path,
                &tag_name,
                &deploy_repo.repo_url,
                &git_env_deploy,
                &log_tx,
            )
            .await?;
        }
    } else {
        let _ = log_tx.send("No deploy changes detected; skipping git commit/push/tag".to_string());
    }

    let commit_sha = if job.dry_run {
        None
    } else {
        get_git_head_sha(&deploy_repo_path, &git_env_deploy).await.ok()
    };

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
    env_name: &str,
    release: &Release,
    manifest_path: &FsPath,
    env_repo_path: &FsPath,
    mapped_vars: &HashMap<String, String>,
    extra_env_vars: &[DeployTargetExtraEnvVarInput],
) -> anyhow::Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    env.insert(
        "ENVIRONMENTS_DIR".to_string(),
        env_repo_path.to_string_lossy().to_string(),
    );
    env.insert("SIMPLE_RELEASE_ID".to_string(), release.release_id.clone());
    env.insert(
        "SRM_RELEASE_MANIFEST".to_string(),
        manifest_path.to_string_lossy().to_string(),
    );
    env.insert("BUILD_ENV".to_string(), env_name.to_string());
    for (key, value) in mapped_vars {
        env.insert(key.clone(), value.clone());
    }
    for item in extra_env_vars {
        let key = item.key.trim();
        if key.is_empty() {
            continue;
        }
        env.insert(key.to_string(), item.value.clone());
    }
    Ok(env)
}

async fn build_env_file(
    state: &DeployApiState,
    environment: &Environment,
    encjson_key_dir: Option<&str>,
    env_repo_path: &FsPath,
    env_subdir: &str,
    env_file_path: &FsPath,
    release: &Release,
    env_vars: &[DeployTargetEnvVarInput],
    extra_env_vars: &[DeployTargetExtraEnvVarInput],
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let env_dir = env_repo_path.join(env_subdir);
    let secured = env_dir.join("env.secured.json");
    let unsecured = env_dir.join("env.unsecured.json");

    let mut combined = String::new();

    let key_dir_override = encjson_key_dir
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from);
    let key_dir_override = key_dir_override.as_ref().map(|p| p.as_path());

    if secured.exists() {
        let output = run_encjson_dotenv(state, environment, &secured, log_tx, key_dir_override).await?;
        combined.push_str(&output);
    }

    if unsecured.exists() {
        let output = run_encjson_dotenv(state, environment, &unsecured, log_tx, key_dir_override).await?;
        combined.push_str(&output);
    }

    combined.push_str(&format!("SIMPLE_RELEASE_ID={}\n", release.release_id));
    for (key, value) in build_release_env_var_map(env_vars, release, log_tx) {
        combined.push_str(&format!("{}={}\n", key, value));
    }
    for item in extra_env_vars {
        let key = item.key.trim();
        if key.is_empty() {
            continue;
        }
        combined.push_str(&format!("{}={}\n", key, item.value.trim()));
    }

    tokio::fs::write(env_file_path, combined)
        .await
        .with_context(|| format!("Failed to write env file {}", env_file_path.display()))?;
    Ok(())
}

async fn load_deploy_target_env_vars(pool: &PgPool, deploy_target_id: Uuid) -> anyhow::Result<Vec<DeployTargetEnvVar>> {
    let rows = sqlx::query_as::<_, DeployTargetEnvVar>(
        "SELECT * FROM deploy_target_env_vars WHERE deploy_target_id = $1 ORDER BY target_key",
    )
    .bind(deploy_target_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn load_deploy_target_extra_env_vars(
    pool: &PgPool,
    deploy_target_id: Uuid,
) -> anyhow::Result<Vec<DeployTargetExtraEnvVar>> {
    let rows = sqlx::query_as::<_, DeployTargetExtraEnvVar>(
        "SELECT * FROM deploy_target_extra_env_vars WHERE deploy_target_id = $1 ORDER BY key",
    )
    .bind(deploy_target_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

fn build_release_env_var_map(
    env_vars: &[DeployTargetEnvVarInput],
    release: &Release,
    log_tx: &broadcast::Sender<String>,
) -> HashMap<String, String> {
    let mut mapped = HashMap::new();
    for item in env_vars {
        let source_key = item.source_key.trim();
        let target_key = item.target_key.trim();
        if source_key.is_empty() || target_key.is_empty() {
            continue;
        }
        let value = match source_key {
            "SIMPLE_RELEASE_ID" => Some(release.release_id.clone()),
            _ => None,
        };
        if let Some(val) = value {
            mapped.insert(target_key.to_string(), val);
        } else {
            let _ = log_tx.send(format!(
                "Release env mapping ignored (unsupported source_key): {} -> {}",
                source_key, target_key
            ));
        }
    }
    mapped
}

async fn run_encjson_dotenv(
    state: &DeployApiState,
    environment: &Environment,
    file_path: &FsPath,
    log_tx: &broadcast::Sender<String>,
    keydir_override: Option<&FsPath>,
) -> anyhow::Result<String> {
    let contents = tokio::fs::read_to_string(file_path)
        .await
        .with_context(|| format!("Failed to read env file {}", file_path.display()))?;
    let api_mode = detect_encjson_api(&contents);

    match api_mode {
        EncJsonApi::Legacy => {
            let _ = log_tx.send(format!(
                "encjson legacy detected in {}, using legacy pipeline",
                file_path.display()
            ));
            run_encjson_legacy_pipeline(state, environment, file_path, keydir_override).await
        }
        EncJsonApi::Modern => {
            let _ = log_tx.send(format!(
                "encjson modern detected in {}, using encjson-rs",
                file_path.display()
            ));
            run_encjson_modern(state, environment, file_path, keydir_override).await
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncJsonApi {
    Legacy,
    Modern,
}

fn detect_encjson_api(contents: &str) -> EncJsonApi {
    if contents.contains("EncJson[@api=1.0") {
        EncJsonApi::Legacy
    } else if contents.contains("EncJson[@api=2.0") {
        EncJsonApi::Modern
    } else {
        EncJsonApi::Modern
    }
}

async fn run_encjson_modern(
    state: &DeployApiState,
    environment: &Environment,
    file_path: &FsPath,
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
    } else if let Some(keydir) = &environment.encjson_key_dir {
        cmd.arg("-k").arg(keydir);
    }

    let output = cmd.output().await?;
    if !output.status.success() {
        anyhow::bail!("encjson-rs failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn run_encjson_legacy_pipeline(
    state: &DeployApiState,
    environment: &Environment,
    file_path: &FsPath,
    keydir_override: Option<&FsPath>,
) -> anyhow::Result<String> {
    let mut legacy_cmd = Command::new(&state.encjson_legacy_path);
    legacy_cmd
        .arg("decrypt")
        .arg("-f")
        .arg(file_path)
        .stdout(std::process::Stdio::piped());

    if let Some(keydir) = keydir_override {
        legacy_cmd.arg("-k").arg(keydir);
    } else if let Some(keydir) = &environment.encjson_key_dir {
        legacy_cmd.arg("-k").arg(keydir);
    }

    let mut legacy_child = legacy_cmd.spawn()?;
    let mut legacy_stdout = legacy_child
        .stdout
        .take()
        .context("Failed to capture legacy encjson stdout")?;

    let mut modern_cmd = Command::new(&state.encjson_path);
    modern_cmd
        .arg("decrypt")
        .arg("-o")
        .arg("dot-env")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());

    let mut modern_child = modern_cmd.spawn()?;
    let mut modern_stdin = modern_child
        .stdin
        .take()
        .context("Failed to capture encjson-rs stdin")?;

    tokio::io::copy(&mut legacy_stdout, &mut modern_stdin).await?;
    drop(modern_stdin);

    let output = modern_child.wait_with_output().await?;
    let legacy_status = legacy_child.wait().await?;

    if !legacy_status.success() {
        anyhow::bail!("encjson legacy failed");
    }
    if !output.status.success() {
        anyhow::bail!("encjson-rs failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn build_encjson_keydir(
    state: &DeployApiState,
    deploy_target_id: Uuid,
    encjson_key_dir: Option<&str>,
    temp_root: &FsPath,
) -> anyhow::Result<Option<PathBuf>> {
    if encjson_key_dir.unwrap_or("").is_empty() {
        let keys = sqlx::query_as::<_, DeployTargetEncjsonKey>(
            "SELECT * FROM deploy_target_encjson_keys WHERE deploy_target_id = $1 ORDER BY created_at",
        )
        .bind(deploy_target_id)
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

async fn apply_release_manifest_mode(
    mode: &str,
    manifest: &mut ReleaseManifest,
    env_repo_root: &FsPath,
    env_name: &str,
    env_repo_path: Option<&str>,
) -> anyhow::Result<()> {
    let normalized = mode.trim().to_lowercase();
    let strict = normalized.starts_with("strict");
    let tag_only = normalized.ends_with("tag");
    let digest_required = normalized.ends_with("digest") && strict;

    if strict {
        let expected = load_env_app_container_pairs(env_repo_root, env_name, env_repo_path).await?;
        let actual: HashSet<(String, String)> = manifest
            .images
            .iter()
            .map(|img| (img.app_name.clone(), img.container_name.clone().unwrap_or_default()))
            .collect();
        let missing: Vec<String> = expected
            .difference(&actual)
            .map(|(app, cont)| {
                if cont.is_empty() {
                    app.to_string()
                } else {
                    format!("{}:{}", app, cont)
                }
            })
            .collect();
        if !missing.is_empty() {
            return Err(anyhow::anyhow!(
                "Release manifest is missing mappings for: {}",
                missing.join(", ")
            ));
        }
    }

    for img in &mut manifest.images {
        if tag_only {
            img.digest = None;
        }
        if digest_required && img.digest.as_deref().unwrap_or("").is_empty() {
            return Err(anyhow::anyhow!(
                "Release manifest requires digest for {}:{}",
                img.app_name,
                img.container_name.clone().unwrap_or_else(|| "-".to_string())
            ));
        }
        if normalized == "strict_tag" && img.tag.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Release manifest requires tag for {}:{}",
                img.app_name,
                img.container_name.clone().unwrap_or_else(|| "-".to_string())
            ));
        }
    }

    Ok(())
}

async fn load_env_app_container_pairs(
    env_repo_root: &FsPath,
    env_name: &str,
    env_repo_path: Option<&str>,
) -> anyhow::Result<HashSet<(String, String)>> {
    let env_subdir = env_repo_path.unwrap_or(env_name).trim().trim_start_matches('/');
    let apps_dir = env_repo_root.join(env_subdir).join("apps");
    let mut defaults = Vec::new();
    if apps_dir.exists() {
        for entry in WalkDir::new(&apps_dir).max_depth(1) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('_') && (name.ends_with(".yml") || name.ends_with(".yaml")) {
                defaults.push(entry.path().to_path_buf());
            }
        }
    }

    let mut default_content = String::new();
    for file in defaults {
        if let Ok(text) = tokio::fs::read_to_string(&file).await {
            default_content.push_str("\n");
            default_content.push_str(&text);
        }
    }

    let mut pairs = HashSet::new();
    for entry in WalkDir::new(&apps_dir).max_depth(1) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name.starts_with('_') || !(name.ends_with(".yml") || name.ends_with(".yaml")) {
            continue;
        }
        let raw = tokio::fs::read_to_string(entry.path()).await?;
        let mut content = format!("{}{}", raw, default_content);
        content = apply_env_placeholders(&content);
        let vars = extract_vars(&content);
        for (key, value) in vars {
            content = apply_var_placeholder(&content, &key, &value);
        }
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
        let app_name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if app_name.is_empty() {
            continue;
        }
        let mut containers = Vec::new();
        if let Some(list) = yaml.get("containers").and_then(|v| v.as_sequence()) {
            for item in list {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    containers.push(name.to_string());
                }
            }
        }
        if containers.is_empty() {
            pairs.insert((app_name, String::new()));
        } else {
            for c in containers {
                pairs.insert((app_name.clone(), c));
            }
        }
    }

    Ok(pairs)
}

fn apply_env_placeholders(content: &str) -> String {
    let mut result = content.to_string();
    for (key, value) in std::env::vars() {
        result = apply_placeholder(&result, "env", &key, &value);
    }
    result
}

fn extract_vars(content: &str) -> Vec<(String, String)> {
    let Ok(yaml) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content) else {
        return Vec::new();
    };
    let Some(vars) = yaml.get("vars").and_then(|v| v.as_sequence()) else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    for item in vars {
        let key = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let value = item.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !key.is_empty() {
            pairs.push((key, value));
        }
    }
    pairs
}

fn apply_var_placeholder(content: &str, key: &str, value: &str) -> String {
    apply_placeholder(content, "var", key, value)
}

fn apply_placeholder(content: &str, kind: &str, key: &str, value: &str) -> String {
    let mut result = content.to_string();
    let patterns = [
        format!("{{{{{kind}:{key}}}}}"),
        format!("{{{{ {kind}:{key} }}}}"),
        format!("{{{{{kind}: {key}}}}}"),
        format!("{{{{ {kind}: {key} }}}}"),
    ];
    for pattern in patterns {
        result = result.replace(&pattern, value);
    }
    result
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

    let intent_out = Command::new("git")
        .arg("add")
        .arg("-N")
        .arg("--")
        .arg(add_path)
        .current_dir(repo_path)
        .output()
        .await?;
    if !intent_out.status.success() {
        let _ = log_tx.send("git add -N failed (continuing)".to_string());
    }

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

async fn collect_and_store_deploy_images(
    pool: &PgPool,
    job_id: Uuid,
    deploy_path: &FsPath,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let rows = collect_deploy_images(deploy_path, log_tx).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM deploy_job_images WHERE deploy_job_id = $1")
        .bind(job_id)
        .execute(&mut *tx)
        .await?;

    if rows.is_empty() {
        let _ = log_tx.send("No deploy images detected (deployments folder empty)".to_string());
        tx.commit().await?;
        return Ok(());
    }

    for row in rows {
        sqlx::query(
            "INSERT INTO deploy_job_images (deploy_job_id, file_path, container_name, image) VALUES ($1, $2, $3, $4)",
        )
        .bind(job_id)
        .bind(row.file_path)
        .bind(row.container_name)
        .bind(row.image)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn collect_deploy_images(
    deploy_path: &FsPath,
    log_tx: &broadcast::Sender<String>,
) -> anyhow::Result<Vec<DeployJobImageRow>> {
    let deployments_dir = deploy_path.join("deployments");
    if !deployments_dir.exists() {
        let _ = log_tx.send("Deployments directory not found; skipping image list".to_string());
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    for entry in WalkDir::new(&deployments_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if ext != "yml" && ext != "yaml" {
            continue;
        }

        let content = match tokio::fs::read_to_string(path).await {
            Ok(text) => text,
            Err(err) => {
                let _ = log_tx.send(format!("Failed to read {}: {}", path.display(), err));
                continue;
            }
        };

        let rel_path = path
            .strip_prefix(deploy_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        for doc in serde_yaml_ng::Deserializer::from_str(&content) {
            let value = match YamlValue::deserialize(doc) {
                Ok(val) => val,
                Err(err) => {
                    let _ = log_tx.send(format!("Failed to parse YAML {}: {}", path.display(), err));
                    continue;
                }
            };

            let mut containers = Vec::new();
            collect_containers_from_doc(&value, &mut containers);
            for (name, image) in containers {
                rows.push(DeployJobImageRow {
                    file_path: rel_path.clone(),
                    container_name: name,
                    image,
                });
            }
        }
    }

    Ok(rows)
}

fn collect_containers_from_doc(doc: &YamlValue, output: &mut Vec<(String, String)>) {
    if let Some(kind) = yaml_string(doc.get("kind")) {
        if kind == "List" {
            if let Some(items) = doc.get("items").and_then(|v| v.as_sequence()) {
                for item in items {
                    collect_containers_from_doc(item, output);
                }
            }
            return;
        }
    }

    let paths: &[&[&str]] = &[
        &["spec", "template", "spec"],
        &["spec", "jobTemplate", "spec", "template", "spec"],
        &["spec"],
    ];

    for path in paths {
        if let Some(spec) = yaml_at_path(doc, path) {
            collect_containers_from_spec(spec, output);
        }
    }
}

fn collect_containers_from_spec(spec: &YamlValue, output: &mut Vec<(String, String)>) {
    for key in ["containers", "initContainers"] {
        if let Some(seq) = spec.get(key).and_then(|v| v.as_sequence()) {
            for item in seq {
                let name = yaml_string(item.get("name")).unwrap_or("").trim().to_string();
                let image = yaml_string(item.get("image")).unwrap_or("").trim().to_string();
                if !name.is_empty() && !image.is_empty() {
                    output.push((name, image));
                }
            }
        }
    }
}

fn yaml_at_path<'a>(value: &'a YamlValue, path: &[&str]) -> Option<&'a YamlValue> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn yaml_string(value: Option<&YamlValue>) -> Option<&str> {
    value.and_then(|v| v.as_str())
}

async fn deploy_job_logs_sse(
    State(state): State<DeployApiState>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    let receiver = {
        let logs = state.job_logs.read().await;
        logs.get(&job_id).map(|sender| sender.subscribe())
    };

    let Some(rx) = receiver else {
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

async fn deploy_job_images(
    State(state): State<DeployApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Vec<DeployJobImageRow>>, (StatusCode, Json<ErrorResponse>)> {
    let rows = sqlx::query_as::<_, DeployJobImageRow>(
        "SELECT file_path, container_name, image FROM deploy_job_images WHERE deploy_job_id = $1 ORDER BY file_path, container_name",
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to load deploy job images: {}", e),
            }),
        )
    })?;

    Ok(Json(rows))
}

#![allow(dead_code)]

use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, BoxStream, Stream, StreamExt};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::crypto;
use crate::db::models::{Bundle, CopyJobImage, Environment, ImageMapping, Registry};
use crate::services::skopeo::SkopeoCredentials;
use crate::services::SkopeoService;

/// Request pro spuštění copy operace
#[derive(Debug, Deserialize)]
pub struct CopyBundleRequest {
    pub target_tag: Option<String>,
    pub timezone_offset_minutes: Option<i32>,
    pub environment_id: Option<Uuid>,
    pub source_registry_id: Option<Uuid>,
    pub target_registry_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PrecheckRequest {
    pub environment_id: Option<Uuid>,
    pub source_registry_id: Option<Uuid>,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Response s job ID
#[derive(Debug, Serialize)]
pub struct CopyJobResponse {
    pub job_id: Uuid,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct NextTagQuery {
    pub tz_offset_minutes: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct NextTagResponse {
    pub tag: String,
}

#[derive(Debug, Deserialize)]
pub struct CompareCopyJobsQuery {
    pub job_a: Uuid,
    pub job_b: Uuid,
}

#[derive(Debug, Serialize)]
pub struct CompareCopyJobsRow {
    pub app_name: String,
    pub container_name: String,
    pub digest_a: Option<String>,
    pub digest_b: Option<String>,
    pub status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct CopyJobDigestRow {
    app_name: String,
    container_name: String,
    source_sha256: Option<String>,
    target_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PrecheckResult {
    pub total: usize,
    pub ok: usize,
    pub failed: Vec<PrecheckFailure>,
}

#[derive(Debug, Serialize)]
pub struct PrecheckFailure {
    pub source_image: String,
    pub source_tag: String,
    pub error: String,
}

/// Status copy jobu
#[derive(Debug, Clone, Serialize)]
pub struct CopyJobStatus {
    pub job_id: Uuid,
    pub bundle_id: Uuid,
    pub version: i32,
    pub status: String,
    pub source_registry_id: Option<Uuid>,
    pub target_registry_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub target_tag: String,
    pub is_release_job: bool,
    pub is_selective: bool,
    pub base_copy_job_id: Option<Uuid>,
    pub validate_only: bool,
    pub total_images: usize,
    pub copied_images: usize,
    pub failed_images: usize,
    pub current_image: Option<String>,
}

/// Shrnutý záznam copy jobu
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CopyJobSummary {
    pub job_id: Uuid,
    pub bundle_id: Uuid,
    pub bundle_name: String,
    pub version: i32,
    pub target_tag: String,
    pub status: String,
    pub is_release_job: bool,
    pub is_selective: bool,
    pub base_copy_job_id: Option<Uuid>,
    pub validate_only: bool,
    pub source_registry_id: Option<Uuid>,
    pub target_registry_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseCopyRequest {
    pub source_copy_job_id: Uuid,
    pub target_registry_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub release_id: String,
    pub notes: Option<String>,
    pub source_ref_mode: Option<String>,
    pub source_tag_override: Option<String>,
    pub validate_only: Option<bool>,
    pub extra_tags: Option<Vec<String>>,
    pub rename_rules: Vec<RenameRule>,
    pub overrides: Vec<ImageOverride>,
}

#[derive(Debug, Deserialize)]
pub struct SelectiveCopyRequest {
    pub base_copy_job_id: Uuid,
    pub selected_image_ids: Vec<Uuid>,
    pub target_tag: Option<String>,
    pub timezone_offset_minutes: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct RenameRule {
    pub find: String,
    pub replace: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageOverride {
    pub copy_job_image_id: Uuid,
    pub override_name: String,
}

/// App state pro copy API
#[derive(Clone)]
pub struct CopyApiState {
    pub pool: PgPool,
    pub skopeo: SkopeoService,
    pub encryption_secret: String,
    pub job_logs: Arc<RwLock<std::collections::HashMap<Uuid, broadcast::Sender<String>>>>,
    pub cancel_flags: Arc<RwLock<HashSet<Uuid>>>,
}

impl CopyApiState {
    /// Získá dešifrované credentials pro registry
    async fn get_registry_credentials(
        &self,
        registry_id: Uuid,
        environment_id: Option<Uuid>,
    ) -> Result<(Option<String>, Option<String>), anyhow::Error> {
        if let Some(env_id) = environment_id {
            let env_row = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
                r#"
                SELECT
                    source_registry_id,
                    target_registry_id,
                    source_auth_type,
                    source_username,
                    source_password_encrypted,
                    source_token_encrypted,
                    target_auth_type,
                    target_username,
                    target_password_encrypted,
                    target_token_encrypted
                FROM environments WHERE id = $1
                "#,
            )
            .bind(env_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some((source_registry_id, target_registry_id, source_auth_type, source_username, source_password_encrypted, source_token_encrypted, target_auth_type, target_username, target_password_encrypted, target_token_encrypted)) = env_row {
                if source_registry_id == Some(registry_id) {
                    if let Some(auth_type) = source_auth_type {
                        return Ok(decrypt_registry_credentials(
                            &auth_type,
                            source_username,
                            source_password_encrypted,
                            source_token_encrypted,
                            &self.encryption_secret,
                        )?);
                    }
                }
                if target_registry_id == Some(registry_id) {
                    if let Some(auth_type) = target_auth_type {
                        return Ok(decrypt_registry_credentials(
                            &auth_type,
                            target_username,
                            target_password_encrypted,
                            target_token_encrypted,
                            &self.encryption_secret,
                        )?);
                    }
                }
            }

            let env_creds = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
                "SELECT auth_type, username, password_encrypted, token_encrypted FROM environment_registry_credentials WHERE registry_id = $1 AND environment_id = $2",
            )
            .bind(registry_id)
            .bind(env_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some((auth_type, username, password_encrypted, token_encrypted)) = env_creds {
                return Ok(decrypt_registry_credentials(
                    &auth_type,
                    username,
                    password_encrypted,
                    token_encrypted,
                    &self.encryption_secret,
                )?);
            }
        }

        let registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
            .bind(registry_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(registry) = registry else {
            return Ok((None, None));
        };

        decrypt_registry_credentials(
            &registry.auth_type,
            registry.username.clone(),
            registry.password_encrypted.clone(),
            registry.token_encrypted.clone(),
            &self.encryption_secret,
        )
    }

    /// Vytvoří SkopeoCredentials pro copy operaci mezi source a target registry
    async fn get_skopeo_credentials(
        &self,
        source_registry_id: Uuid,
        target_registry_id: Uuid,
        environment_id: Option<Uuid>,
    ) -> Result<SkopeoCredentials, anyhow::Error> {
        let (source_username, source_password) = self
            .get_registry_credentials(source_registry_id, environment_id)
            .await?;
        let (target_username, target_password) = self
            .get_registry_credentials(target_registry_id, environment_id)
            .await?;

        Ok(SkopeoCredentials {
            source_username,
            source_password,
            target_username,
            target_password,
        })
    }
}

fn decrypt_registry_credentials(
    auth_type: &str,
    username: Option<String>,
    password_encrypted: Option<String>,
    token_encrypted: Option<String>,
    encryption_secret: &str,
) -> Result<(Option<String>, Option<String>), anyhow::Error> {
    match auth_type {
        "none" => Ok((None, None)),
        "basic" => {
            let password = if let Some(encrypted) = &password_encrypted {
                Some(crypto::decrypt(encrypted, encryption_secret)?)
            } else {
                None
            };
            Ok((username, password))
        }
        "token" => {
            let token = if let Some(encrypted) = &token_encrypted {
                Some(crypto::decrypt(encrypted, encryption_secret)?)
            } else {
                None
            };
            Ok((username, token))
        }
        "bearer" => {
            let token = if let Some(encrypted) = &token_encrypted {
                Some(crypto::decrypt(encrypted, encryption_secret)?)
            } else {
                None
            };
            Ok((None, token))
        }
        _ => Ok((None, None)),
    }
}

/// Vytvoří router pro copy endpoints
pub fn router(state: CopyApiState) -> Router {
    Router::new()
        .route(
            "/bundles/{bundle_id}/versions/{version}/copy",
            post(copy_bundle_version),
        )
        .route(
            "/bundles/{bundle_id}/versions/{version}/next-tag",
            get(get_next_copy_tag),
        )
        .route(
            "/bundles/{bundle_id}/versions/{version}/precheck",
            post(precheck_copy_images),
        )
        .route("/copy/jobs", get(list_copy_jobs))
        .route("/copy/jobs/compare", get(compare_copy_jobs))
        .route("/copy/jobs/release/precheck", post(precheck_release_copy_images))
        .route("/copy/jobs/release", post(start_release_copy_job))
        .route("/copy/jobs/selective", post(start_selective_copy_job))
        .route("/copy/jobs/{job_id}/start", post(start_copy_job))
        .route("/copy/jobs/{job_id}/cancel", post(cancel_copy_job))
        .route("/copy/jobs/{job_id}", get(get_copy_job_status))
        .route("/copy/jobs/{job_id}/images", get(get_copy_job_images))
        .route("/copy/jobs/{job_id}/logs", get(copy_job_logs_sse))
        .route("/copy/jobs/{job_id}/logs/history", get(copy_job_logs_history))
        .route("/copy/jobs/{job_id}/progress", get(copy_job_progress_sse))
        .with_state(state)
}

fn local_date_from_offset(offset_minutes: Option<i32>) -> NaiveDate {
    let offset = offset_minutes.unwrap_or(0) as i64;
    let local = Utc::now() - Duration::minutes(offset);
    local.date_naive()
}

fn format_tag(date: NaiveDate, counter: i32) -> String {
    format!(
        "{:04}.{:02}.{:02}.{:02}",
        date.year(),
        date.month(),
        date.day(),
        counter
    )
}

fn apply_rename_rules(mut path: String, rules: &[RenameRule]) -> String {
    for rule in rules {
        if !rule.find.is_empty() {
            path = path.replace(&rule.find, &rule.replace);
        }
    }
    path
}

fn apply_override_name(path: &str, override_name: &str) -> String {
    if override_name.is_empty() {
        return path.to_string();
    }
    if let Some((prefix, _)) = path.rsplit_once('/') {
        format!("{}/{}", prefix, override_name)
    } else {
        override_name.to_string()
    }
}

fn apply_registry_project_path(path: &str, default_project_path: Option<&str>) -> String {
    let Some(default_path) = default_project_path.map(str::trim).filter(|p| !p.is_empty()) else {
        return path.to_string();
    };
    let default_path = default_path.trim_matches('/');
    if default_path.is_empty() {
        return path.to_string();
    }
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return default_path.to_string();
    }
    let rest = trimmed.split_once('/').map(|(_, rest)| rest).unwrap_or(trimmed);
    format!("{}/{}", default_path, rest)
}

async fn resolve_registry_project_path(
    pool: &PgPool,
    registry_id: Uuid,
    environment_id: Option<Uuid>,
    role: &str,
) -> Result<Option<String>, sqlx::Error> {
    if let Some(env_id) = environment_id {
        let override_path: Option<String> = sqlx::query_scalar(
            r#"
            SELECT project_path_override
            FROM environment_registry_paths
            WHERE registry_id = $1 AND environment_id = $2 AND role = $3
            "#,
        )
        .bind(registry_id)
        .bind(env_id)
        .bind(role)
        .fetch_optional(pool)
        .await?
        .flatten();

        if override_path.is_some() {
            return Ok(override_path);
        }
    }

    sqlx::query_scalar::<_, Option<String>>(
        "SELECT default_project_path FROM registries WHERE id = $1",
    )
    .bind(registry_id)
    .fetch_optional(pool)
    .await
    .map(|v| v.flatten())
}

fn build_source_url(base: &str, img: &CopyJobImage, mode: &str) -> Result<String, String> {
    if mode == "digest" {
        if let Some(digest) = img.source_sha256.as_deref() {
            if !digest.trim().is_empty() {
                return Ok(format!("{}/{}@{}", base, img.source_image, digest));
            }
        }
        return Err(format!(
            "Missing source digest for {}:{}",
            img.source_image, img.source_tag
        ));
    }
    Ok(format!("{}/{}:{}", base, img.source_image, img.source_tag))
}

async fn is_registry_enabled_for_env(
    pool: &PgPool,
    registry_id: Uuid,
    environment_id: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    let Some(env_id) = environment_id else {
        return Ok(true);
    };
    let enabled = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT COALESCE(era.is_enabled, TRUE)
        FROM environments e
        LEFT JOIN environment_registry_access era
          ON era.environment_id = e.id AND era.registry_id = $1
        WHERE e.id = $2
        "#,
    )
    .bind(registry_id)
    .bind(env_id)
    .fetch_one(pool)
    .await?;
    Ok(enabled)
}

fn emit_log(
    log_tx: &broadcast::Sender<String>,
    line: String,
) {
    let _ = log_tx.send(line.clone());
}

/// POST /api/v1/bundles/{bundle_id}/versions/{version}/copy - Spustí copy operaci
async fn copy_bundle_version(
    State(state): State<CopyApiState>,
    Path((bundle_id, version)): Path<(Uuid, i32)>,
    Json(payload): Json<CopyBundleRequest>,
) -> Result<(StatusCode, Json<CopyJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Získat bundle
    let bundle = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles WHERE id = $1")
        .bind(bundle_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Bundle with id {} not found", bundle_id),
                }),
            )
        })?;

    // Získat bundle_version_id
    let bundle_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM bundle_versions WHERE bundle_id = $1 AND version = $2",
    )
    .bind(bundle_id)
    .bind(version)
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
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Bundle version {} not found", version),
            }),
        )
    })?;

    // Získat všechny image mappings
    let mappings = sqlx::query_as::<_, ImageMapping>(
        "SELECT * FROM image_mappings WHERE bundle_version_id = $1 ORDER BY created_at",
    )
    .bind(bundle_version_id)
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

    if mappings.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No image mappings found for this bundle version".to_string(),
            }),
        ));
    }

    // Získat registries pro URL construction
    if payload.environment_id.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is required".to_string(),
            }),
        ));
    }
    let environment_id = payload.environment_id;

    let environment = sqlx::query_as::<_, Environment>("SELECT * FROM environments WHERE id = $1")
        .bind(environment_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Environment not found".to_string(),
                }),
            )
        })?;

    let source_registry_id = environment
        .source_registry_id
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment is missing source registry".to_string(),
                }),
            )
        })?;
    let target_registry_id = environment
        .target_registry_id
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment is missing target registry".to_string(),
                }),
            )
        })?;

    let _source_registry: (String,) = sqlx::query_as(
        "SELECT base_url FROM registries WHERE id = $1",
    )
    .bind(source_registry_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get source registry: {}", e),
            }),
        )
    })?;

    let _target_registry: (String,) = sqlx::query_as(
        "SELECT base_url FROM registries WHERE id = $1",
    )
    .bind(target_registry_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get target registry: {}", e),
            }),
        )
    })?;

    let target_tag = if bundle.auto_tag_enabled {
        let date = local_date_from_offset(payload.timezone_offset_minutes);
        let counter: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO bundle_tag_counters (bundle_id, target_registry_id, date, counter)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (bundle_id, target_registry_id, date)
            DO UPDATE SET counter = bundle_tag_counters.counter + 1, updated_at = now()
            RETURNING counter
            "#,
        )
        .bind(bundle.id)
        .bind(target_registry_id)
        .bind(date)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to generate target tag: {}", e),
                }),
            )
        })?;
        format_tag(date, counter)
    } else {
        let tag = payload.target_tag.clone().unwrap_or_default().trim().to_string();
        if tag.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Target tag is required".to_string(),
                }),
            ));
        }
        tag
    };

    // Vytvořit job
    if payload.environment_id.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is required".to_string(),
            }),
        ));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO copy_jobs (id, bundle_version_id, target_tag, status, source_registry_id, target_registry_id, environment_id)
         VALUES ($1, $2, $3, 'pending', $4, $5, $6)"
    )
    .bind(job_id)
    .bind(bundle_version_id)
    .bind(&target_tag)
    .bind(source_registry_id)
    .bind(target_registry_id)
    .bind(payload.environment_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create copy job: {}", e),
            }),
        )
    })?;

    let source_project_path = environment.source_project_path.clone();
    let target_project_path = environment.target_project_path.clone();

    // Vytvořit snapshot image mappings pro tento job
    for mapping in &mappings {
        let source_path =
            apply_registry_project_path(&mapping.source_image, source_project_path.as_deref());
        let target_path =
            apply_registry_project_path(&mapping.target_image, target_project_path.as_deref());
        let _copy_job_image_id: Uuid = sqlx::query_scalar(
            "INSERT INTO copy_job_images
             (copy_job_id, image_mapping_id, source_image, source_tag, target_image, target_tag, copy_status)
             VALUES ($1, $2, $3, $4, $5, $6, 'pending')
             RETURNING id"
        )
        .bind(job_id)
        .bind(mapping.id)
        .bind(&source_path)
        .bind(&mapping.source_tag)
        .bind(&target_path)
        .bind(&target_tag)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to snapshot image mappings: {}", e),
                }),
            )
        })?;
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: format!("Copy job created for {} images", mappings.len()),
        }),
    ))
}

/// GET /api/v1/bundles/{bundle_id}/versions/{version}/next-tag - Náhled dalšího tagu
async fn get_next_copy_tag(
    State(state): State<CopyApiState>,
    Path((bundle_id, _version)): Path<(Uuid, i32)>,
    Query(query): Query<NextTagQuery>,
) -> Result<Json<NextTagResponse>, (StatusCode, Json<ErrorResponse>)> {
    let bundle = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles WHERE id = $1")
        .bind(bundle_id)
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

    let Some(bundle) = bundle else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Bundle with id {} not found", bundle_id),
            }),
        ));
    };

    if !bundle.auto_tag_enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Auto tag is not enabled for this bundle".to_string(),
            }),
        ));
    }

    let date = local_date_from_offset(query.tz_offset_minutes);
    let current: Option<i32> = sqlx::query_scalar(
        "SELECT counter FROM bundle_tag_counters WHERE bundle_id = $1 AND target_registry_id = $2 AND date = $3",
    )
    .bind(bundle.id)
    .bind(bundle.target_registry_id)
    .bind(date)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get tag counter: {}", e),
            }),
        )
    })?;

    let next = current.unwrap_or(0) + 1;
    let tag = format_tag(date, next);

    Ok(Json(NextTagResponse { tag }))
}

/// POST /api/v1/bundles/{bundle_id}/versions/{version}/precheck - ověří zdrojové images
async fn precheck_copy_images(
    State(state): State<CopyApiState>,
    Path((bundle_id, version)): Path<(Uuid, i32)>,
    Json(payload): Json<PrecheckRequest>,
) -> Result<Json<PrecheckResult>, (StatusCode, Json<ErrorResponse>)> {
    let _bundle = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles WHERE id = $1")
        .bind(bundle_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Bundle {} not found", bundle_id),
                }),
            )
        })?;

    let mappings = sqlx::query_as::<_, ImageMapping>(
        r#"
        SELECT im.*
        FROM image_mappings im
        JOIN bundle_versions bv ON bv.id = im.bundle_version_id
        WHERE bv.bundle_id = $1 AND bv.version = $2
        ORDER BY im.created_at
        "#,
    )
    .bind(bundle_id)
    .bind(version)
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

    if mappings.is_empty() {
        return Ok(Json(PrecheckResult {
            total: 0,
            ok: 0,
            failed: vec![],
        }));
    }

    let environment_id = payload.environment_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is required".to_string(),
            }),
        )
    })?;

    let environment = sqlx::query_as::<_, Environment>("SELECT * FROM environments WHERE id = $1")
        .bind(environment_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Environment not found".to_string(),
                }),
            )
        })?;

    let source_registry_id = environment
        .source_registry_id
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment is missing source registry".to_string(),
                }),
            )
        })?;

    let source_registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
        .bind(source_registry_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Source registry not found".to_string(),
                }),
            )
        })?;

    let (source_username, source_password) = state
        .get_registry_credentials(source_registry_id, Some(environment_id))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get registry credentials: {}", e),
                }),
            )
        })?;

    let source_base_url = source_registry
        .base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();

    let source_project_path = environment.source_project_path.clone();

    let total = mappings.len();
    let mut failed = Vec::new();

    for mapping in mappings {
        let source_path =
            apply_registry_project_path(&mapping.source_image, source_project_path.as_deref());
        let source_url = format!(
            "{}/{}:{}",
            source_base_url, source_path, mapping.source_tag
        );
        let result = state
            .skopeo
            .inspect_image(&source_url, source_username.as_deref(), source_password.as_deref())
            .await;
        if let Err(err) = result {
            failed.push(PrecheckFailure {
                source_image: source_path,
                source_tag: mapping.source_tag,
                error: err.to_string(),
            });
        }
    }

    let ok = total - failed.len();
    Ok(Json(PrecheckResult { total, ok, failed }))
}

/// GET /api/v1/copy/jobs/{job_id}/images - seznam image výsledků pro job
async fn get_copy_job_images(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Vec<CopyJobImage>>, (StatusCode, Json<ErrorResponse>)> {
    let images = sqlx::query_as::<_, CopyJobImage>(
        "SELECT * FROM copy_job_images WHERE copy_job_id = $1 ORDER BY created_at"
    )
    .bind(job_id)
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

    Ok(Json(images))
}

/// POST /api/v1/copy/jobs/release/precheck - ověří zdrojové images pro release
async fn precheck_release_copy_images(
    State(state): State<CopyApiState>,
    Json(payload): Json<ReleaseCopyRequest>,
) -> Result<Json<PrecheckResult>, (StatusCode, Json<ErrorResponse>)> {
    let source_ref_mode = payload
        .source_ref_mode
        .unwrap_or_else(|| "tag".to_string())
        .to_lowercase();
    let source_ref_mode = match source_ref_mode.as_str() {
        "digest" => "digest".to_string(),
        _ => "tag".to_string(),
    };

    let environment_id = payload.environment_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is required".to_string(),
            }),
        )
    })?;

    let source_tag_override = payload
        .source_tag_override
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    if source_ref_mode == "digest" && source_tag_override.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Source tag override is only allowed in tag mode".to_string(),
            }),
        ));
    }

    let _environment = sqlx::query_as::<_, Environment>("SELECT * FROM environments WHERE id = $1")
        .bind(environment_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Environment not found".to_string(),
                }),
            )
        })?;

    let source_job = sqlx::query_as::<_, (Option<Uuid>,)>(
        "SELECT target_registry_id FROM copy_jobs WHERE id = $1",
    )
    .bind(payload.source_copy_job_id)
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
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Source copy job not found".to_string(),
            }),
        )
    })?;

    let source_registry_id = source_job.0.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Source copy job has no target registry".to_string(),
            }),
        )
    })?;

    let source_registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
        .bind(source_registry_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Source registry not found".to_string(),
                }),
            )
        })?;

    let images = sqlx::query_as::<_, CopyJobImage>(
        "SELECT * FROM copy_job_images WHERE copy_job_id = $1 ORDER BY created_at",
    )
    .bind(payload.source_copy_job_id)
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

    if images.is_empty() {
        return Ok(Json(PrecheckResult {
            total: 0,
            ok: 0,
            failed: vec![],
        }));
    }

    let (source_username, source_password) = state
        .get_registry_credentials(source_registry_id, Some(environment_id))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get registry credentials: {}", e),
                }),
            )
        })?;

    let source_base_url = source_registry
        .base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();

    let total = images.len();
    let mut failed = Vec::new();

    for img in images {
        if source_ref_mode == "digest" && img.target_sha256.as_deref().unwrap_or("").is_empty() {
            failed.push(PrecheckFailure {
                source_image: img.target_image.clone(),
                source_tag: img.target_tag.clone(),
                error: "Missing digest".to_string(),
            });
            continue;
        }

        let source_url = if source_ref_mode == "digest" {
            format!(
                "{}/{}@{}",
                source_base_url,
                img.target_image,
                img.target_sha256.as_deref().unwrap_or("")
            )
        } else {
            let tag = source_tag_override
                .as_deref()
                .unwrap_or(&img.target_tag)
                .to_string();
            format!("{}/{}:{}", source_base_url, img.target_image, tag)
        };

        let result = state
            .skopeo
            .inspect_image(&source_url, source_username.as_deref(), source_password.as_deref())
            .await;
        if let Err(err) = result {
            failed.push(PrecheckFailure {
                source_image: img.target_image.clone(),
                source_tag: img.target_tag.clone(),
                error: err.to_string(),
            });
        }
    }

    let ok = total - failed.len();
    Ok(Json(PrecheckResult { total, ok, failed }))
}

/// POST /api/v1/copy/jobs/release - Spustí release copy job ze zdrojového jobu
async fn start_release_copy_job(
    State(state): State<CopyApiState>,
    Json(payload): Json<ReleaseCopyRequest>,
) -> Result<(StatusCode, Json<CopyJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let release_id = payload.release_id.trim().to_string();
    if release_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Release ID cannot be empty".to_string(),
            }),
        ));
    }

    let source_ref_mode = payload
        .source_ref_mode
        .unwrap_or_else(|| "tag".to_string())
        .to_lowercase();
    let source_ref_mode = match source_ref_mode.as_str() {
        "digest" => "digest".to_string(),
        _ => "tag".to_string(),
    };

    let payload = ReleaseCopyRequest {
        release_id: release_id.clone(),
        source_ref_mode: Some(source_ref_mode.clone()),
        ..payload
    };
    let environment_id = payload.environment_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Environment is required".to_string(),
            }),
        )
    })?;
    let source_tag_override = payload
        .source_tag_override
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let mut extra_tags = payload
        .extra_tags
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .filter(|t| t != &release_id)
        .collect::<Vec<String>>();
    extra_tags.sort();
    extra_tags.dedup();

    if source_ref_mode == "digest" && source_tag_override.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Source tag override is only allowed in tag mode".to_string(),
            }),
        ));
    }

    // Release ID musí být unikátní
    let release_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM releases WHERE release_id = $1)"
    )
    .bind(&release_id)
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

    if release_exists {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("Release with ID '{}' already exists", release_id),
            }),
        ));
    }

    // Zdrojový job musí existovat a být úspěšný
    let source_job = sqlx::query_as::<_, (Uuid, String, Option<Uuid>, Option<Uuid>)>(
        "SELECT bundle_version_id, status, source_registry_id, target_registry_id
         FROM copy_jobs WHERE id = $1"
    )
    .bind(payload.source_copy_job_id)
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

    let Some((bundle_version_id, status, _src_registry_id, src_target_registry_id)) = source_job else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Copy job with id {} not found", payload.source_copy_job_id),
            }),
        ));
    };

    if status != "success" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Source copy job must be successful".to_string(),
            }),
        ));
    }

    // Zjistit source registry (target registry zdrojového jobu)
    let source_registry_id: Uuid = if let Some(id) = src_target_registry_id {
        id
    } else {
        sqlx::query_scalar(
            "SELECT b.target_registry_id
             FROM bundle_versions bv
             JOIN bundles b ON b.id = bv.bundle_id
             WHERE bv.id = $1"
        )
        .bind(bundle_version_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to resolve source registry: {}", e),
                }),
            )
        })?
    };

    // Načíst images ze zdrojového jobu
    let source_images = sqlx::query_as::<_, CopyJobImage>(
        "SELECT * FROM copy_job_images WHERE copy_job_id = $1 ORDER BY created_at"
    )
    .bind(payload.source_copy_job_id)
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

    if source_images.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No images found in source copy job".to_string(),
            }),
        ));
    }

    if source_ref_mode == "digest" {
        let missing: Vec<String> = source_images
            .iter()
            .filter(|img| img.target_sha256.as_deref().unwrap_or("").is_empty())
            .map(|img| format!("{}:{}", img.target_image, img.target_tag))
            .collect();
        if !missing.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "Digest missing for {} images; cannot use digest source mode",
                        missing.len()
                    ),
                }),
            ));
        }
    }

    let environment = sqlx::query_as::<_, Environment>("SELECT * FROM environments WHERE id = $1")
        .bind(environment_id)
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
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Environment not found".to_string(),
                }),
            )
        })?;

    let target_registry_id = environment
        .target_registry_id
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Environment is missing target registry".to_string(),
                }),
            )
        })?;

    let target_project_path = environment.target_project_path.clone();

    let registry_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM registries WHERE id = $1)",
    )
    .bind(target_registry_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to verify target registry: {}", e),
            }),
        )
    })?;

    if !registry_exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Target registry not found".to_string(),
            }),
        ));
    }

    // Připravit override map
    let mut overrides = std::collections::HashMap::new();
    for ov in payload.overrides {
        if !ov.override_name.trim().is_empty() {
            overrides.insert(ov.copy_job_image_id, ov.override_name);
        }
    }

    // Vytvořit nový job
    let job_id = Uuid::new_v4();
    let validate_only = payload.validate_only.unwrap_or(false);

    sqlx::query(
        "INSERT INTO copy_jobs
         (id, bundle_version_id, target_tag, status, source_registry_id, target_registry_id, source_ref_mode, is_release_job, release_id, release_notes, validate_only, environment_id, extra_tags)
         VALUES ($1, $2, $3, 'pending', $4, $5, $6, TRUE, $7, $8, $9, $10, $11)"
    )
    .bind(job_id)
    .bind(bundle_version_id)
    .bind(&release_id)
    .bind(source_registry_id)
    .bind(target_registry_id)
    .bind(&source_ref_mode)
    .bind(&release_id)
    .bind(&payload.notes)
    .bind(validate_only)
    .bind(Some(environment_id))
    .bind(&extra_tags)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create release copy job: {}", e),
            }),
        )
    })?;

    // Snapshot pro nový job
    let mut job_images: Vec<(Uuid, String, String, String)> = Vec::with_capacity(source_images.len());
    let rename_rules = &payload.rename_rules;
    for img in &source_images {
        let mut target_path =
            apply_registry_project_path(&img.target_image, target_project_path.as_deref());
        target_path = apply_rename_rules(target_path, rename_rules);
        if let Some(override_name) = overrides.get(&img.id) {
            target_path = apply_override_name(&target_path, override_name);
        }

        let source_sha = if source_ref_mode == "digest" {
            img.target_sha256.clone()
        } else {
            None
        };
        let source_tag = source_tag_override
            .as_deref()
            .unwrap_or(&img.target_tag)
            .to_string();

        let copy_job_image_id: Uuid = sqlx::query_scalar(
            "INSERT INTO copy_job_images
             (copy_job_id, image_mapping_id, source_image, source_tag, target_image, target_tag, source_sha256)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id"
        )
        .bind(job_id)
        .bind(img.image_mapping_id)
        .bind(&img.target_image)
        .bind(&source_tag)
        .bind(&target_path)
        .bind(&release_id)
        .bind(&source_sha)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to snapshot release images: {}", e),
                }),
            )
        })?;

        job_images.push((
            copy_job_image_id,
            img.target_image.clone(),
            source_tag,
            target_path,
        ));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: format!("Release copy job started for {} images", source_images.len()),
        }),
    ))
}

/// POST /api/v1/copy/jobs/selective - Spustí selective copy job ze zdrojového jobu
async fn start_selective_copy_job(
    State(state): State<CopyApiState>,
    Json(payload): Json<SelectiveCopyRequest>,
) -> Result<(StatusCode, Json<CopyJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    if payload.selected_image_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Select at least one image to update".to_string(),
            }),
        ));
    }

    let base_job = sqlx::query_as::<_, (Uuid, Uuid, String, bool, Option<Uuid>, Option<Uuid>, String, Uuid, bool, Option<Uuid>)>(
        r#"
        SELECT
            cj.id,
            cj.bundle_version_id,
            cj.status,
            cj.is_release_job,
            cj.source_registry_id,
            cj.target_registry_id,
            cj.target_tag,
            b.id AS bundle_id,
            b.auto_tag_enabled,
            cj.environment_id
        FROM copy_jobs cj
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        WHERE cj.id = $1
        "#
    )
    .bind(payload.base_copy_job_id)
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

    let Some((base_job_id, bundle_version_id, status, is_release_job, source_registry_id, target_registry_id, _base_tag, bundle_id, auto_tag_enabled, environment_id)) = base_job else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Base copy job not found".to_string(),
            }),
        ));
    };

    if status != "success" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Base copy job must be successful".to_string(),
            }),
        ));
    }

    if is_release_job {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Selective copy is only allowed for normal copy jobs".to_string(),
            }),
        ));
    }

    let (Some(source_registry_id), Some(target_registry_id)) = (source_registry_id, target_registry_id) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Base copy job is missing registries".to_string(),
            }),
        ));
    };

    let target_tag = if auto_tag_enabled {
        let date = local_date_from_offset(payload.timezone_offset_minutes);
        let counter: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO bundle_tag_counters (bundle_id, target_registry_id, date, counter)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (bundle_id, target_registry_id, date)
            DO UPDATE SET counter = bundle_tag_counters.counter + 1, updated_at = now()
            RETURNING counter
            "#,
        )
        .bind(bundle_id)
        .bind(target_registry_id)
        .bind(date)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to generate target tag: {}", e),
                }),
            )
        })?;
        format_tag(date, counter)
    } else {
        let tag = payload.target_tag.clone().unwrap_or_default().trim().to_string();
        if tag.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Target tag is required".to_string(),
                }),
            ));
        }
        tag
    };

    let base_images = sqlx::query_as::<_, CopyJobImage>(
        "SELECT * FROM copy_job_images WHERE copy_job_id = $1 ORDER BY created_at"
    )
    .bind(base_job_id)
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

    if base_images.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Base copy job has no images".to_string(),
            }),
        ));
    }

    let selected: std::collections::HashSet<Uuid> = payload.selected_image_ids.into_iter().collect();
    let invalid_selected = selected
        .iter()
        .filter(|id| !base_images.iter().any(|img| &img.id == *id))
        .count();
    if invalid_selected > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Some selected images do not belong to the base copy job".to_string(),
            }),
        ));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO copy_jobs
         (id, bundle_version_id, target_tag, status, source_registry_id, target_registry_id, is_selective, base_copy_job_id, environment_id)
         VALUES ($1, $2, $3, 'pending', $4, $5, TRUE, $6, $7)"
    )
    .bind(job_id)
    .bind(bundle_version_id)
    .bind(&target_tag)
    .bind(source_registry_id)
    .bind(target_registry_id)
    .bind(base_job_id)
    .bind(environment_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create selective copy job: {}", e),
            }),
        )
    })?;

    for img in base_images {
        let is_selected = selected.contains(&img.id);
        let (source_image, source_tag, source_registry_override) = if is_selected {
            (img.source_image.clone(), img.source_tag.clone(), Some(source_registry_id))
        } else {
            (img.target_image.clone(), img.target_tag.clone(), Some(target_registry_id))
        };

        let _: Uuid = sqlx::query_scalar(
            "INSERT INTO copy_job_images
             (copy_job_id, image_mapping_id, source_image, source_tag, source_registry_id, target_image, target_tag)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id"
        )
        .bind(job_id)
        .bind(img.image_mapping_id)
        .bind(&source_image)
        .bind(&source_tag)
        .bind(source_registry_override)
        .bind(&img.target_image)
        .bind(&target_tag)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to snapshot selective images: {}", e),
                }),
            )
        })?;
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: format!("Selective copy job created (tag {})", target_tag),
        }),
    ))
}

/// GET /api/v1/copy/jobs - seznam copy jobů
async fn list_copy_jobs(
    State(state): State<CopyApiState>,
) -> Result<Json<Vec<CopyJobSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let jobs = sqlx::query_as::<_, CopyJobSummary>(
        r#"
        SELECT
            cj.id AS job_id,
            bv.bundle_id,
            b.name AS bundle_name,
            bv.version,
            cj.target_tag,
            cj.status,
            cj.is_release_job,
            cj.is_selective,
            cj.base_copy_job_id,
            cj.validate_only,
            cj.source_registry_id,
            cj.target_registry_id,
            cj.environment_id,
            cj.started_at,
            cj.completed_at
        FROM copy_jobs cj
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        ORDER BY cj.started_at DESC
        LIMIT 100
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

/// GET /api/v1/copy/jobs/compare?job_a=...&job_b=... - porovnání digestů mezi dvěma copy joby
async fn compare_copy_jobs(
    State(state): State<CopyApiState>,
    Query(params): Query<CompareCopyJobsQuery>,
) -> Result<Json<Vec<CompareCopyJobsRow>>, (StatusCode, Json<ErrorResponse>)> {
    if params.job_a == params.job_b {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Select two different copy jobs".to_string(),
            }),
        ));
    }

    let rows_a = sqlx::query_as::<_, CopyJobDigestRow>(
        r#"
        SELECT
            im.app_name,
            im.container_name,
            cji.source_sha256,
            cji.target_sha256
        FROM copy_job_images cji
        JOIN image_mappings im ON im.id = cji.image_mapping_id
        WHERE cji.copy_job_id = $1
        "#,
    )
    .bind(params.job_a)
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

    let rows_b = sqlx::query_as::<_, CopyJobDigestRow>(
        r#"
        SELECT
            im.app_name,
            im.container_name,
            cji.source_sha256,
            cji.target_sha256
        FROM copy_job_images cji
        JOIN image_mappings im ON im.id = cji.image_mapping_id
        WHERE cji.copy_job_id = $1
        "#,
    )
    .bind(params.job_b)
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

    if rows_a.is_empty() || rows_b.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Both copy jobs must contain images".to_string(),
            }),
        ));
    }

    let pick_digest = |row: &CopyJobDigestRow| {
        row.target_sha256
            .clone()
            .or_else(|| row.source_sha256.clone())
    };

    let mut map_a: HashMap<(String, String), Option<String>> = HashMap::new();
    for row in rows_a {
        let digest = pick_digest(&row);
        map_a.insert((row.app_name, row.container_name), digest);
    }

    let mut map_b: HashMap<(String, String), Option<String>> = HashMap::new();
    for row in rows_b {
        let digest = pick_digest(&row);
        map_b.insert((row.app_name, row.container_name), digest);
    }

    let mut keys: Vec<(String, String)> = map_a
        .keys()
        .chain(map_b.keys())
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();

    let mut results = Vec::with_capacity(keys.len());
    for (app_name, container_name) in keys {
        let digest_a = map_a
            .get(&(app_name.clone(), container_name.clone()))
            .cloned()
            .flatten();
        let digest_b = map_b
            .get(&(app_name.clone(), container_name.clone()))
            .cloned()
            .flatten();

        let status = match (&digest_a, &digest_b) {
            (Some(a), Some(b)) if a == b => "same",
            (Some(_), Some(_)) => "changed",
            (None, Some(_)) => "missing_in_a",
            (Some(_), None) => "missing_in_b",
            _ => "missing",
        }
        .to_string();

        results.push(CompareCopyJobsRow {
            app_name,
            container_name,
            digest_a,
            digest_b,
            status,
        });
    }

    Ok(Json(results))
}

/// POST /api/v1/copy/jobs/{job_id}/start - Spustí pending copy job
async fn start_copy_job(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<(StatusCode, Json<CopyJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let job = sqlx::query_as::<_, (String, Option<Uuid>, Option<Uuid>, String, String, bool, Option<String>, Option<String>, bool, Option<Uuid>, Option<Vec<String>>)>(
        "SELECT status, source_registry_id, target_registry_id, target_tag, source_ref_mode, is_release_job, release_id, release_notes, validate_only, environment_id, extra_tags
         FROM copy_jobs WHERE id = $1"
    )
    .bind(job_id)
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

    let Some((status, source_registry_id, target_registry_id, target_tag, source_ref_mode, is_release_job, release_id, release_notes, validate_only, environment_id, extra_tags)) = job else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Copy job with id {} not found", job_id),
            }),
        ));
    };

    if status != "pending" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Copy job is not pending".to_string(),
            }),
        ));
    }

    let (Some(source_registry_id), Some(target_registry_id)) = (source_registry_id, target_registry_id) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Copy job does not have source/target registries".to_string(),
            }),
        ));
    };

    let images = sqlx::query_as::<_, CopyJobImage>(
        "SELECT * FROM copy_job_images WHERE copy_job_id = $1 ORDER BY created_at"
    )
    .bind(job_id)
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

    if images.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No images found for this job".to_string(),
            }),
        ));
    }

    if source_ref_mode == "digest" {
        let missing = images
            .iter()
            .filter(|img| img.source_sha256.as_deref().unwrap_or("").is_empty())
            .count();
        if missing > 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("{} images missing source digest for digest mode", missing),
                }),
            ));
        }
    }

    let (log_tx, _log_rx) = broadcast::channel(512);
    state.job_logs.write().await.insert(job_id, log_tx.clone());

    // Persist logs to DB
    let pool_for_log = state.pool.clone();
    let mut log_rx = log_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(line) = log_rx.recv().await {
            let _ = sqlx::query(
                "INSERT INTO copy_job_logs (copy_job_id, line) VALUES ($1, $2)",
            )
            .bind(job_id)
            .bind(line)
            .execute(&pool_for_log)
            .await;
        }
    });

    let pool_clone = state.pool.clone();
    let skopeo_clone = state.skopeo.clone();
    let log_state_clone = state.job_logs.clone();

    let extra_tags = extra_tags.unwrap_or_default();

    let mut source_registry_ids = std::collections::HashSet::new();
    source_registry_ids.insert(source_registry_id);
    for img in &images {
        if let Some(id) = img.source_registry_id {
            source_registry_ids.insert(id);
        }
    }

    let mut source_registry_info: std::collections::HashMap<Uuid, (String, Option<String>, Option<String>)> = std::collections::HashMap::new();
    for registry_id in source_registry_ids {
        let registry: (String,) = sqlx::query_as(
            "SELECT base_url FROM registries WHERE id = $1",
        )
        .bind(registry_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get source registry: {}", e),
                }),
            )
        })?;
        let (username, password) = state
            .get_registry_credentials(registry_id, environment_id)
            .await
            .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get registry credentials: {}", e),
                }),
            )
        })?;
        let base_url = registry
            .0
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();
        source_registry_info.insert(registry_id, (base_url, username, password));
    }

    let target_registry: (String,) = sqlx::query_as(
        "SELECT base_url FROM registries WHERE id = $1",
    )
    .bind(target_registry_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get target registry: {}", e),
            }),
        )
    })?;

    let (target_username, target_password) = state
        .get_registry_credentials(target_registry_id, environment_id)
        .await
        .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get registry credentials: {}", e),
            }),
        )
    })?;

    let target_base_url = target_registry.0.trim_start_matches("https://").trim_start_matches("http://").to_string();
    let release_id = release_id.clone();
    let release_notes = release_notes.clone();
    let source_ref_mode = source_ref_mode.clone();
    let cancel_flags = state.cancel_flags.clone();

    tokio::spawn(async move {
        let mut failed = 0;
        let mut cancelled = false;
        emit_log(&log_tx, format!("Starting copy job {} ({} images)", job_id, images.len()));

        if cancel_flags.read().await.contains(&job_id) {
            cancelled = true;
            emit_log(&log_tx, "Cancel requested, stopping job".to_string());
        }

        let _ = sqlx::query("UPDATE copy_jobs SET status = 'in_progress' WHERE id = $1")
            .bind(job_id)
            .execute(&pool_clone)
            .await;

        for img in images {
            if cancel_flags.read().await.contains(&job_id) {
                cancelled = true;
                emit_log(&log_tx, "Cancel requested, stopping job".to_string());
                break;
            }
            let source_registry_id = img.source_registry_id.unwrap_or(source_registry_id);
            let Some((source_base_url, source_username, source_password)) = source_registry_info.get(&source_registry_id).cloned() else {
                failed += 1;
                let err = format!("Missing source registry {}", source_registry_id);
                emit_log(&log_tx, format!("FAILED {} - {}", img.source_image, err));
                let _ = sqlx::query(
                    "UPDATE copy_job_images
                     SET copy_status = 'failed', error_message = $1
                     WHERE id = $2"
                )
                .bind(err)
                .bind(img.id)
                .execute(&pool_clone)
                .await;
                continue;
            };

            let credentials = SkopeoCredentials {
                source_username: source_username.clone(),
                source_password: source_password.clone(),
                target_username: target_username.clone(),
                target_password: target_password.clone(),
            };

            let source_url = match build_source_url(&source_base_url, &img, &source_ref_mode) {
                Ok(url) => url,
                Err(err) => {
                    failed += 1;
                    emit_log(&log_tx, format!("FAILED {} - {}", img.source_image, err));
                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'failed', error_message = $1
                         WHERE id = $2"
                    )
                    .bind(err)
                    .bind(img.id)
                    .execute(&pool_clone)
                    .await;
                    continue;
                }
            };
            let target_url = format!("{}/{}:{}", target_base_url, img.target_image, &target_tag);

            emit_log(&log_tx, format!("Copying {} -> {}", source_url, target_url));

            let _ = sqlx::query("UPDATE copy_job_images SET copy_status = 'in_progress' WHERE id = $1")
                .bind(img.id)
                .execute(&pool_clone)
                .await;

            let source_sha = match skopeo_clone.inspect_image(
                &source_url,
                credentials.source_username.as_deref(),
                credentials.source_password.as_deref(),
            ).await {
                Ok(info) => Some(info.digest),
                Err(_) => None,
            };

            if validate_only {
                if source_sha.is_some() {
                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'success',
                             source_sha256 = $1,
                             copied_at = NOW(),
                             bytes_copied = 0
                         WHERE id = $2"
                    )
                    .bind(&source_sha)
                    .bind(img.id)
                    .execute(&pool_clone)
                    .await;
                    emit_log(&log_tx, format!("VALIDATED {} (source digest ok)", source_url));
                } else {
                    failed += 1;
                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'failed',
                             error_message = 'Source inspect failed',
                             copied_at = NOW()
                         WHERE id = $1"
                    )
                    .bind(img.id)
                    .execute(&pool_clone)
                    .await;
                    emit_log(&log_tx, format!("FAILED {} - Source inspect failed", source_url));
                }
                continue;
            }

            if let Some(ref src_digest) = source_sha {
                match skopeo_clone.inspect_image(
                    &target_url,
                    credentials.target_username.as_deref(),
                    credentials.target_password.as_deref(),
                ).await {
                    Ok(info) => {
                        if info.digest == *src_digest {
                            let _ = sqlx::query(
                                "UPDATE copy_job_images
                                 SET copy_status = 'success',
                                     source_sha256 = $1,
                                     target_sha256 = $2,
                                     copied_at = NOW(),
                                     bytes_copied = 0
                                 WHERE id = $3"
                            )
                            .bind(&source_sha)
                            .bind(&info.digest)
                            .bind(img.id)
                            .execute(&pool_clone)
                            .await;

                            emit_log(&log_tx, format!("SKIP {} (digest match)", target_url));
                            if !extra_tags.is_empty() {
                                for tag in &extra_tags {
                                    if tag == &target_tag {
                                        continue;
                                    }
                                    let extra_target_url =
                                        format!("{}/{}:{}", target_base_url, img.target_image, tag);
                                    emit_log(&log_tx, format!("Tagging {} -> {}", source_url, extra_target_url));
                                    match skopeo_clone
                                        .copy_image_with_retry(
                                            &source_url,
                                            &extra_target_url,
                                            &credentials,
                                            3,
                                            10,
                                            Some(&log_tx),
                                        )
                                        .await
                                    {
                                        Ok(progress) if progress.status == crate::services::skopeo::CopyStatus::Success => {
                                            emit_log(&log_tx, format!("TAGGED {}", extra_target_url));
                                        }
                                        Ok(progress) => {
                                            failed += 1;
                                            emit_log(&log_tx, format!("FAILED {} - {}", extra_target_url, progress.message.trim()));
                                            let _ = sqlx::query(
                                                "UPDATE copy_job_images
                                                 SET copy_status = 'failed', error_message = $1
                                                 WHERE id = $2"
                                            )
                                            .bind(format!("Extra tag {} failed: {}", tag, progress.message.trim()))
                                            .bind(img.id)
                                            .execute(&pool_clone)
                                            .await;
                                        }
                                        Err(err) => {
                                            failed += 1;
                                            emit_log(&log_tx, format!("FAILED {} - {}", extra_target_url, err));
                                            let _ = sqlx::query(
                                                "UPDATE copy_job_images
                                                 SET copy_status = 'failed', error_message = $1
                                                 WHERE id = $2"
                                            )
                                            .bind(format!("Extra tag {} failed: {}", tag, err))
                                            .bind(img.id)
                                            .execute(&pool_clone)
                                            .await;
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                    }
                    Err(err) => {
                        emit_log(&log_tx, format!("WARN target inspect failed for {} ({}) - copying anyway", target_url, err));
                    }
                }
            }

            match skopeo_clone
                .copy_image_with_retry(
                    &source_url,
                    &target_url,
                    &credentials,
                    3,
                    10,
                    Some(&log_tx),
                )
                .await
            {
                Ok(progress) if progress.status == crate::services::skopeo::CopyStatus::Success => {
                    let target_sha = match skopeo_clone.inspect_image(
                        &target_url,
                        credentials.target_username.as_deref(),
                        credentials.target_password.as_deref(),
                    ).await {
                        Ok(info) => Some(info.digest),
                        Err(_) => None,
                    };

                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'success',
                             source_sha256 = $1,
                             target_sha256 = $2,
                             copied_at = NOW()
                         WHERE id = $3"
                    )
                    .bind(&source_sha)
                    .bind(&target_sha)
                    .bind(img.id)
                    .execute(&pool_clone)
                    .await;

                    emit_log(&log_tx, format!("SUCCESS {}", target_url));
                    if !extra_tags.is_empty() {
                        for tag in &extra_tags {
                            if tag == &target_tag {
                                continue;
                            }
                            let extra_target_url =
                                format!("{}/{}:{}", target_base_url, img.target_image, tag);
                            emit_log(&log_tx, format!("Tagging {} -> {}", source_url, extra_target_url));
                            match skopeo_clone
                                .copy_image_with_retry(
                                    &source_url,
                                    &extra_target_url,
                                    &credentials,
                                    3,
                                    10,
                                    Some(&log_tx),
                                )
                                .await
                            {
                                Ok(progress) if progress.status == crate::services::skopeo::CopyStatus::Success => {
                                    emit_log(&log_tx, format!("TAGGED {}", extra_target_url));
                                }
                                Ok(progress) => {
                                    failed += 1;
                                    emit_log(&log_tx, format!("FAILED {} - {}", extra_target_url, progress.message.trim()));
                                    let _ = sqlx::query(
                                        "UPDATE copy_job_images
                                         SET copy_status = 'failed', error_message = $1
                                         WHERE id = $2"
                                    )
                                    .bind(format!("Extra tag {} failed: {}", tag, progress.message.trim()))
                                    .bind(img.id)
                                    .execute(&pool_clone)
                                    .await;
                                }
                                Err(err) => {
                                    failed += 1;
                                    emit_log(&log_tx, format!("FAILED {} - {}", extra_target_url, err));
                                    let _ = sqlx::query(
                                        "UPDATE copy_job_images
                                         SET copy_status = 'failed', error_message = $1
                                         WHERE id = $2"
                                    )
                                    .bind(format!("Extra tag {} failed: {}", tag, err))
                                    .bind(img.id)
                                    .execute(&pool_clone)
                                    .await;
                                }
                            }
                        }
                    }
                }
                Ok(progress) => {
                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'failed', error_message = $1, source_sha256 = $2
                         WHERE id = $3"
                    )
                    .bind(progress.message.trim())
                    .bind(&source_sha)
                    .bind(img.id)
                    .execute(&pool_clone)
                    .await;

                    failed += 1;
                    emit_log(&log_tx, format!("FAILED {} - {}", target_url, progress.message.trim()));
                }
                Err(err) => {
                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'failed', error_message = $1, source_sha256 = $2
                         WHERE id = $3"
                    )
                    .bind(err.to_string())
                    .bind(&source_sha)
                    .bind(img.id)
                    .execute(&pool_clone)
                    .await;

                    failed += 1;
                    emit_log(&log_tx, format!("FAILED {} - {}", target_url, err));
                }
            }
        }

        if cancelled {
            let _ = sqlx::query(
                "UPDATE copy_jobs SET status = 'cancelled', completed_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .execute(&pool_clone)
            .await;

            let _ = sqlx::query(
                "UPDATE copy_job_images
                 SET copy_status = 'cancelled', error_message = 'Cancelled'
                 WHERE copy_job_id = $1 AND copy_status IN ('pending', 'in_progress')",
            )
            .bind(job_id)
            .execute(&pool_clone)
            .await;
        } else {
            let _ = sqlx::query(
                "UPDATE copy_jobs
                 SET status = $1, completed_at = NOW()
                 WHERE id = $2"
            )
            .bind(if failed == 0 { "success" } else { "failed" })
            .bind(job_id)
            .execute(&pool_clone)
            .await;
        }

        if !cancelled && failed == 0 && is_release_job {
            if let Some(release_id) = release_id {
                let _ = sqlx::query(
                    "INSERT INTO releases (copy_job_id, release_id, status, source_ref_mode, notes, is_auto, extra_tags)
                     VALUES ($1, $2, 'draft', $3, $4, false, $5)"
                )
                .bind(job_id)
                .bind(&release_id)
                .bind(&source_ref_mode)
                .bind(&release_notes)
                .bind(&extra_tags)
                .execute(&pool_clone)
                .await;
            }
        }

        emit_log(&log_tx, "Copy job finished".to_string());
        log_state_clone.write().await.remove(&job_id);
        cancel_flags.write().await.remove(&job_id);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: "Copy job started".to_string(),
        }),
    ))
}

/// POST /api/v1/copy/jobs/{job_id}/cancel - Zruší copy job
async fn cancel_copy_job(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<(StatusCode, Json<CopyJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM copy_jobs WHERE id = $1",
    )
    .bind(job_id)
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

    let Some(status) = status else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Copy job with id {} not found", job_id),
            }),
        ));
    };

    if status == "success" || status == "failed" || status == "cancelled" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Copy job is already finished".to_string(),
            }),
        ));
    }

    let _ = sqlx::query(
        "UPDATE copy_jobs SET status = 'cancelled', completed_at = NOW() WHERE id = $1",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await;

    let _ = sqlx::query(
        "UPDATE copy_job_images
         SET copy_status = 'cancelled', error_message = 'Cancelled'
         WHERE copy_job_id = $1 AND copy_status IN ('pending', 'in_progress')",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await;

    state.cancel_flags.write().await.insert(job_id);
    if let Some(sender) = state.job_logs.read().await.get(&job_id) {
        let _ = sender.send("Cancel requested".to_string());
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: "Cancel requested".to_string(),
        }),
    ))
}

/// GET /api/v1/copy/jobs/{job_id} - Získá status copy jobu
async fn get_copy_job_status(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<CopyJobStatus>, (StatusCode, Json<ErrorResponse>)> {
    let row = sqlx::query_as::<_, (Uuid, i32, String, Option<Uuid>, Option<Uuid>, Option<Uuid>, String, bool, bool, Option<Uuid>, bool)>(
        r#"
        SELECT bv.bundle_id, bv.version, cj.status, cj.source_registry_id, cj.target_registry_id, cj.environment_id, cj.target_tag, cj.is_release_job, cj.is_selective, cj.base_copy_job_id, cj.validate_only
        FROM copy_jobs cj
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        WHERE cj.id = $1
        "#
    )
    .bind(job_id)
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

    let Some((bundle_id, version, status, source_registry_id, target_registry_id, environment_id, target_tag, is_release_job, is_selective, base_copy_job_id, validate_only)) = row else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Copy job with id {} not found", job_id),
            }),
        ));
    };

    let totals = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE copy_status = 'success') AS copied,
            COUNT(*) FILTER (WHERE copy_status = 'failed') AS failed
        FROM copy_job_images
        WHERE copy_job_id = $1
        "#
    )
    .bind(job_id)
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

    let current_image = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT CONCAT(source_image, ':', source_tag)
        FROM copy_job_images
        WHERE copy_job_id = $1 AND copy_status = 'in_progress'
        ORDER BY created_at
        LIMIT 1
        "#
    )
    .bind(job_id)
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
    .flatten();

    Ok(Json(CopyJobStatus {
        job_id,
        bundle_id,
        version,
        status,
        source_registry_id,
        target_registry_id,
        environment_id,
        target_tag,
        is_release_job,
        is_selective,
        base_copy_job_id,
        validate_only,
        total_images: totals.0 as usize,
        copied_images: totals.1 as usize,
        failed_images: totals.2 as usize,
        current_image,
    }))
}

/// GET /api/v1/copy/jobs/{job_id}/progress - SSE stream pro real-time progress
async fn copy_job_progress_sse(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::unfold(state, move |state| async move {
        let status = get_copy_job_status(State(state.clone()), Path(job_id)).await.ok();

        match status {
            Some(status) => {
                let json = serde_json::to_string(&status.0).unwrap_or_default();
                let event = Event::default().data(json);

                if status.0.status == "success" || status.0.status == "failed" {
                    Some((Ok(event), state))
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    Some((Ok(event), state))
                }
            }
            None => {
                let error_event = Event::default().data(r#"{"error":"Job not found"}"#);
                Some((Ok(error_event), state))
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// GET /api/v1/copy/jobs/{job_id}/logs - SSE stream s logy ze skopeo
async fn copy_job_logs_sse(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let sender = {
        let logs = state.job_logs.read().await;
        logs.get(&job_id).cloned()
    };

    let stream: BoxStream<'static, Result<Event, Infallible>> = if let Some(sender) = sender {
        let rx = sender.subscribe();
        stream::unfold(rx, |mut rx| async move {
            match rx.recv().await {
                Ok(line) => {
                    let event = Event::default().data(line);
                    Some((Ok(event), rx))
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let event = Event::default().data("[log] ...");
                    Some((Ok(event), rx))
                }
                Err(broadcast::error::RecvError::Closed) => None,
            }
        })
        .boxed()
    } else {
        stream::once(async {
            Ok(Event::default().event("log-end").data("Log stream not available"))
        })
        .boxed()
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// GET /api/v1/copy/jobs/{job_id}/logs/history - celé uložené logy
async fn copy_job_logs_history(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    let lines = sqlx::query_scalar::<_, String>(
        "SELECT line FROM copy_job_logs WHERE copy_job_id = $1 ORDER BY created_at",
    )
    .bind(job_id)
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

    Ok(Json(lines))
}

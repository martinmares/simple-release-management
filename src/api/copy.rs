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
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::crypto;
use crate::db::models::{Bundle, CopyJobImage, ImageMapping, Registry};
use crate::services::skopeo::SkopeoCredentials;
use crate::services::SkopeoService;

/// Request pro spuštění copy operace
#[derive(Debug, Deserialize)]
pub struct CopyBundleRequest {
    pub target_tag: Option<String>,
    pub timezone_offset_minutes: Option<i32>,
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
    pub target_tag: String,
    pub is_release_job: bool,
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
    pub validate_only: bool,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseCopyRequest {
    pub source_copy_job_id: Uuid,
    pub target_registry_id: Uuid,
    pub release_id: String,
    pub notes: Option<String>,
    pub source_ref_mode: Option<String>,
    pub validate_only: Option<bool>,
    pub rename_rules: Vec<RenameRule>,
    pub overrides: Vec<ImageOverride>,
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
    ) -> Result<(Option<String>, Option<String>), anyhow::Error> {
        let registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
            .bind(registry_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(registry) = registry else {
            return Ok((None, None));
        };

        // Decrypt credentials based on auth_type
        match registry.auth_type.as_str() {
            "none" => Ok((None, None)),
            "basic" => {
                let username = registry.username.clone();
                let password = if let Some(encrypted) = &registry.password_encrypted {
                    Some(crypto::decrypt(encrypted, &self.encryption_secret)?)
                } else {
                    None
                };
                Ok((username, password))
            }
            "token" => {
                let username = registry.username.clone();
                let token = if let Some(encrypted) = &registry.token_encrypted {
                    Some(crypto::decrypt(encrypted, &self.encryption_secret)?)
                } else {
                    None
                };
                Ok((username, token))
            }
            "bearer" => {
                let token = if let Some(encrypted) = &registry.token_encrypted {
                    Some(crypto::decrypt(encrypted, &self.encryption_secret)?)
                } else {
                    None
                };
                // For bearer, username is empty
                Ok((None, token))
            }
            _ => Ok((None, None)),
        }
    }

    /// Vytvoří SkopeoCredentials pro copy operaci mezi source a target registry
    async fn get_skopeo_credentials(
        &self,
        source_registry_id: Uuid,
        target_registry_id: Uuid,
    ) -> Result<SkopeoCredentials, anyhow::Error> {
        let (source_username, source_password) = self.get_registry_credentials(source_registry_id).await?;
        let (target_username, target_password) = self.get_registry_credentials(target_registry_id).await?;

        Ok(SkopeoCredentials {
            source_username,
            source_password,
            target_username,
            target_password,
        })
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
        .route("/copy/jobs/release", post(start_release_copy_job))
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
    let source_registry: (String,) = sqlx::query_as(
        "SELECT base_url FROM registries WHERE id = $1",
    )
    .bind(bundle.source_registry_id)
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

    let target_registry: (String,) = sqlx::query_as(
        "SELECT base_url FROM registries WHERE id = $1",
    )
    .bind(bundle.target_registry_id)
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

    let source_base_url = source_registry.0.trim_start_matches("https://").trim_start_matches("http://").to_string();
    let target_base_url = target_registry.0.trim_start_matches("https://").trim_start_matches("http://").to_string();

    // Získat credentials pro source a target registry
    let credentials = match state.get_skopeo_credentials(bundle.source_registry_id, bundle.target_registry_id).await {
        Ok(creds) => creds,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get registry credentials: {}", e),
                }),
            ));
        }
    };

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
        .bind(bundle.target_registry_id)
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
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO copy_jobs (id, bundle_version_id, target_tag, status, source_registry_id, target_registry_id)
         VALUES ($1, $2, $3, 'pending', $4, $5)"
    )
    .bind(job_id)
    .bind(bundle_version_id)
    .bind(&target_tag)
    .bind(bundle.source_registry_id)
    .bind(bundle.target_registry_id)
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

    // Inicializovat log stream pro tento job
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

    // Spustit copy operaci na pozadí
    let pool_clone = state.pool.clone();
    let skopeo_clone = state.skopeo.clone();
    let log_state_clone = state.job_logs.clone();
    let cancel_flags = state.cancel_flags.clone();
    let target_tag = target_tag.clone();
    let credentials_clone = credentials.clone();

    // Vytvořit snapshot image mappings pro tento job
    let mut job_images: Vec<(Uuid, ImageMapping)> = Vec::with_capacity(mappings.len());
    for mapping in &mappings {
        let copy_job_image_id: Uuid = sqlx::query_scalar(
            "INSERT INTO copy_job_images
             (copy_job_id, image_mapping_id, source_image, source_tag, target_image, target_tag)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id"
        )
        .bind(job_id)
        .bind(mapping.id)
        .bind(&mapping.source_image)
        .bind(&mapping.source_tag)
        .bind(&mapping.target_image)
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

        job_images.push((copy_job_image_id, mapping.clone()));
    }

    let mapping_count = mappings.len();

    tokio::spawn(async move {
        let mut failed = 0;
        let mut cancelled = false;

        emit_log(&log_tx, format!("Starting copy job {} ({} images)", job_id, mapping_count));
        if cancel_flags.read().await.contains(&job_id) {
            cancelled = true;
            emit_log(&log_tx, "Cancel requested, stopping job".to_string());
        }
        if !cancelled {
            let _ = sqlx::query("UPDATE copy_jobs SET status = 'in_progress' WHERE id = $1")
                .bind(job_id)
                .execute(&pool_clone)
                .await;
        }

        for (copy_job_image_id, mapping) in job_images {
            if cancel_flags.read().await.contains(&job_id) {
                cancelled = true;
                emit_log(&log_tx, "Cancel requested, stopping job".to_string());
                break;
            }
            // Sestavit URL
            let source_url = format!("{}/{}:{}", source_base_url, mapping.source_image, mapping.source_tag);
            let target_url = format!("{}/{}:{}", target_base_url, mapping.target_image, &target_tag);

            emit_log(&log_tx, format!("Copying {} -> {}", source_url, target_url));

            // Update DB status na in_progress
            let _ = sqlx::query("UPDATE copy_job_images SET copy_status = 'in_progress' WHERE id = $1")
                .bind(copy_job_image_id)
                .execute(&pool_clone)
                .await;

            let source_sha = match skopeo_clone.inspect_image(
                &source_url,
                credentials_clone.source_username.as_deref(),
                credentials_clone.source_password.as_deref(),
            ).await {
                Ok(info) => Some(info.digest),
                Err(_) => None,
            };

            if let Some(ref src_digest) = source_sha {
                match skopeo_clone.inspect_image(
                    &target_url,
                    credentials_clone.target_username.as_deref(),
                    credentials_clone.target_password.as_deref(),
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
                            .bind(copy_job_image_id)
                            .execute(&pool_clone)
                            .await;

                            emit_log(&log_tx, format!("SKIP {} (digest match)", target_url));
                            continue;
                        }
                    }
                    Err(err) => {
                        emit_log(&log_tx, format!("WARN target inspect failed for {} ({}) - copying anyway", target_url, err));
                    }
                }
            }

            // Zkopírovat image
            match skopeo_clone
                .copy_image_with_retry(
                    &source_url,
                    &target_url,
                    &credentials_clone,
                    3,
                    10,
                    Some(&log_tx),
                )
                .await
            {
                Ok(progress) if progress.status == crate::services::skopeo::CopyStatus::Success => {
                    // Získat target SHA
                    let target_sha = match skopeo_clone.inspect_image(
                        &target_url,
                        credentials_clone.target_username.as_deref(),
                        credentials_clone.target_password.as_deref(),
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
                    .bind(copy_job_image_id)
                    .execute(&pool_clone)
                    .await;

                    emit_log(&log_tx, format!("SUCCESS {}", target_url));
                }
                Ok(progress) => {
                    // Update DB na failed
                    let _ = sqlx::query(
                        "UPDATE copy_job_images
                         SET copy_status = 'failed', error_message = $1, source_sha256 = $2
                         WHERE id = $3"
                    )
                    .bind(progress.message.trim())
                    .bind(&source_sha)
                    .bind(copy_job_image_id)
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
                    .bind(copy_job_image_id)
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
            // Finalizovat job
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

        emit_log(&log_tx, "Copy job finished".to_string());
        log_state_clone.write().await.remove(&job_id);
        cancel_flags.write().await.remove(&job_id);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: format!("Copy job started for {} images", mapping_count),
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
) -> Result<Json<PrecheckResult>, (StatusCode, Json<ErrorResponse>)> {
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

    let source_registry = sqlx::query_as::<_, Registry>("SELECT * FROM registries WHERE id = $1")
        .bind(bundle.source_registry_id)
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
        .get_registry_credentials(bundle.source_registry_id)
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

    let total = mappings.len();
    let mut failed = Vec::new();

    for mapping in mappings {
        let source_url = format!(
            "{}/{}:{}",
            source_base_url, mapping.source_image, mapping.source_tag
        );
        let result = state
            .skopeo
            .inspect_image(&source_url, source_username.as_deref(), source_password.as_deref())
            .await;
        if let Err(err) = result {
            failed.push(PrecheckFailure {
                source_image: mapping.source_image,
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
         (id, bundle_version_id, target_tag, status, source_registry_id, target_registry_id, source_ref_mode, is_release_job, release_id, release_notes, validate_only)
         VALUES ($1, $2, $3, 'pending', $4, $5, $6, TRUE, $7, $8, $9)"
    )
    .bind(job_id)
    .bind(bundle_version_id)
    .bind(&release_id)
    .bind(source_registry_id)
    .bind(payload.target_registry_id)
    .bind(&source_ref_mode)
    .bind(&release_id)
    .bind(&payload.notes)
    .bind(validate_only)
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
        let mut target_path = apply_rename_rules(img.target_image.clone(), rename_rules);
        if let Some(override_name) = overrides.get(&img.id) {
            target_path = apply_override_name(&target_path, override_name);
        }

        let source_sha = if source_ref_mode == "digest" {
            img.target_sha256.clone()
        } else {
            None
        };

        let copy_job_image_id: Uuid = sqlx::query_scalar(
            "INSERT INTO copy_job_images
             (copy_job_id, image_mapping_id, source_image, source_tag, target_image, target_tag, source_sha256)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id"
        )
        .bind(job_id)
        .bind(img.image_mapping_id)
        .bind(&img.target_image)
        .bind(&img.target_tag)
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
            img.target_tag.clone(),
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
            cj.validate_only,
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

/// POST /api/v1/copy/jobs/{job_id}/start - Spustí pending copy job
async fn start_copy_job(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<(StatusCode, Json<CopyJobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let job = sqlx::query_as::<_, (String, Option<Uuid>, Option<Uuid>, String, String, bool, Option<String>, Option<String>, bool)>(
        "SELECT status, source_registry_id, target_registry_id, target_tag, source_ref_mode, is_release_job, release_id, release_notes, validate_only
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

    let Some((status, source_registry_id, target_registry_id, target_tag, source_ref_mode, is_release_job, release_id, release_notes, validate_only)) = job else {
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
    let credentials = state.get_skopeo_credentials(source_registry_id, target_registry_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get registry credentials: {}", e),
            }),
        )
    })?;

    let source_registry: (String,) = sqlx::query_as(
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

    let source_base_url = source_registry.0.trim_start_matches("https://").trim_start_matches("http://").to_string();
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
                    "INSERT INTO releases (copy_job_id, release_id, status, source_ref_mode, notes, is_auto)
                     VALUES ($1, $2, 'draft', $3, $4, false)"
                )
                .bind(job_id)
                .bind(&release_id)
                .bind(&source_ref_mode)
                .bind(&release_notes)
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
    let row = sqlx::query_as::<_, (Uuid, i32, String, Option<Uuid>, Option<Uuid>, String, bool, bool)>(
        r#"
        SELECT bv.bundle_id, bv.version, cj.status, cj.source_registry_id, cj.target_registry_id, cj.target_tag, cj.is_release_job, cj.validate_only
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

    let Some((bundle_id, version, status, source_registry_id, target_registry_id, target_tag, is_release_job, validate_only)) = row else {
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
        target_tag,
        is_release_job,
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

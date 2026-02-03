use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, BoxStream, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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
    pub target_tag: String,
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

/// Status copy jobu
#[derive(Debug, Clone, Serialize)]
pub struct CopyJobStatus {
    pub job_id: Uuid,
    pub bundle_id: Uuid,
    pub version: i32,
    pub status: String,
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
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// App state pro copy API
#[derive(Clone)]
pub struct CopyApiState {
    pub pool: PgPool,
    pub skopeo: SkopeoService,
    pub encryption_secret: String,
    pub job_logs: Arc<RwLock<std::collections::HashMap<Uuid, broadcast::Sender<String>>>>,
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
        .route("/copy/jobs", get(list_copy_jobs))
        .route("/copy/jobs/{job_id}", get(get_copy_job_status))
        .route("/copy/jobs/{job_id}/images", get(get_copy_job_images))
        .route("/copy/jobs/{job_id}/logs", get(copy_job_logs_sse))
        .route("/copy/jobs/{job_id}/progress", get(copy_job_progress_sse))
        .with_state(state)
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

    // Vytvořit job
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO copy_jobs (id, bundle_version_id, target_tag, status)
         VALUES ($1, $2, $3, 'pending')"
    )
    .bind(job_id)
    .bind(bundle_version_id)
    .bind(&payload.target_tag)
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

    // Spustit copy operaci na pozadí
    let pool_clone = state.pool.clone();
    let skopeo_clone = state.skopeo.clone();
    let target_tag = payload.target_tag.clone();
    let credentials_clone = credentials.clone();
    let log_state_clone = state.job_logs.clone();

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

        let _ = log_tx.send(format!("Starting copy job {} ({} images)", job_id, mapping_count));
        let _ = sqlx::query("UPDATE copy_jobs SET status = 'in_progress' WHERE id = $1")
            .bind(job_id)
            .execute(&pool_clone)
            .await;

        for (copy_job_image_id, mapping) in job_images {
            // Sestavit URL
            let source_url = format!("{}/{}:{}", source_base_url, mapping.source_image, mapping.source_tag);
            let target_url = format!("{}/{}:{}", target_base_url, mapping.target_image, &target_tag);

            let _ = log_tx.send(format!("Copying {} -> {}", source_url, target_url));

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

                    let _ = log_tx.send(format!("SUCCESS {}", target_url));
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
                    let _ = log_tx.send(format!("FAILED {} - {}", target_url, progress.message.trim()));
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
                    let _ = log_tx.send(format!("FAILED {} - {}", target_url, err));
                }
            }

        }

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

        let _ = log_tx.send("Copy job finished".to_string());
        log_state_clone.write().await.remove(&job_id);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: format!("Copy job started for {} images", mapping_count),
        }),
    ))
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

/// GET /api/v1/copy/jobs/{job_id} - Získá status copy jobu
async fn get_copy_job_status(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<CopyJobStatus>, (StatusCode, Json<ErrorResponse>)> {
    let row = sqlx::query_as::<_, (Uuid, i32, String)>(
        r#"
        SELECT bv.bundle_id, bv.version, cj.status
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

    let Some((bundle_id, version, status)) = row else {
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

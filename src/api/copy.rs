use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::crypto;
use crate::db::models::{Bundle, ImageMapping, Registry};
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

/// Shared state pro copy jobs
#[derive(Clone)]
pub struct CopyJobState {
    pub jobs: Arc<RwLock<std::collections::HashMap<Uuid, CopyJobStatus>>>,
}

impl CopyJobState {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

/// App state pro copy API
#[derive(Clone)]
pub struct CopyApiState {
    pub pool: PgPool,
    pub skopeo: SkopeoService,
    pub job_state: CopyJobState,
    pub encryption_secret: String,
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
        .route("/copy/jobs/{job_id}", get(get_copy_job_status))
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

    // Vytvořit job ID
    let job_id = Uuid::new_v4();

    // Inicializovat job status
    let job_status = CopyJobStatus {
        job_id,
        bundle_id,
        version,
        status: "starting".to_string(),
        total_images: mappings.len(),
        copied_images: 0,
        failed_images: 0,
        current_image: None,
    };

    state.job_state.jobs.write().await.insert(job_id, job_status.clone());

    // Spustit copy operaci na pozadí
    let pool_clone = state.pool.clone();
    let skopeo_clone = state.skopeo.clone();
    let job_state_clone = state.job_state.clone();
    let target_tag = payload.target_tag.clone();
    let credentials_clone = credentials.clone();

    tokio::spawn(async move {
        let mut copied = 0;
        let mut failed = 0;

        for mapping in mappings {
            // Update current image
            if let Some(jobs) = job_state_clone.jobs.write().await.get_mut(&job_id) {
                jobs.current_image = Some(format!("{}:{}", mapping.source_image, mapping.source_tag));
                jobs.status = "in_progress".to_string();
            }

            // Sestavit URL
            let source_url = format!("{}/{}:{}", source_base_url, mapping.source_image, mapping.source_tag);
            let target_url = format!("{}/{}:{}", target_base_url, mapping.target_image, &target_tag);

            // Update DB status na in_progress
            let _ = sqlx::query("UPDATE image_mappings SET copy_status = 'in_progress' WHERE id = $1")
                .bind(mapping.id)
                .execute(&pool_clone)
                .await;

            // Zkopírovat image
            match skopeo_clone.copy_image_with_retry(&source_url, &target_url, &credentials_clone, 3, 10).await {
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

                    // Update DB
                    let _ = sqlx::query(
                        "UPDATE image_mappings SET copy_status = 'success', target_sha256 = $1, copied_at = NOW() WHERE id = $2"
                    )
                    .bind(&target_sha)
                    .bind(mapping.id)
                    .execute(&pool_clone)
                    .await;

                    copied += 1;
                }
                _ => {
                    // Update DB na failed
                    let _ = sqlx::query(
                        "UPDATE image_mappings SET copy_status = 'failed', error_message = $1 WHERE id = $2"
                    )
                    .bind("Copy operation failed")
                    .bind(mapping.id)
                    .execute(&pool_clone)
                    .await;

                    failed += 1;
                }
            }

            // Update job status
            if let Some(jobs) = job_state_clone.jobs.write().await.get_mut(&job_id) {
                jobs.copied_images = copied;
                jobs.failed_images = failed;
            }
        }

        // Finalizovat job
        if let Some(jobs) = job_state_clone.jobs.write().await.get_mut(&job_id) {
            jobs.status = if failed == 0 { "completed".to_string() } else { "completed_with_errors".to_string() };
            jobs.current_image = None;
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CopyJobResponse {
            job_id,
            message: format!("Copy job started for {} images", job_status.total_images),
        }),
    ))
}

/// GET /api/v1/copy/jobs/{job_id} - Získá status copy jobu
async fn get_copy_job_status(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<CopyJobStatus>, (StatusCode, Json<ErrorResponse>)> {
    let jobs = state.job_state.jobs.read().await;

    match jobs.get(&job_id) {
        Some(status) => Ok(Json(status.clone())),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Copy job with id {} not found", job_id),
            }),
        )),
    }
}

/// GET /api/v1/copy/jobs/{job_id}/progress - SSE stream pro real-time progress
async fn copy_job_progress_sse(
    State(state): State<CopyApiState>,
    Path(job_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::unfold(state, move |state| async move {
        // Zkontrolovat jestli job existuje
        let status = {
            let jobs = state.job_state.jobs.read().await;
            jobs.get(&job_id).cloned()
        };

        match status {
            Some(status) => {
                let json = serde_json::to_string(&status).unwrap_or_default();
                let event = Event::default().data(json);

                // Pokud je job dokončený, ukončit stream
                if status.status == "completed" || status.status == "completed_with_errors" {
                    Some((Ok(event), state))
                } else {
                    // Počkat chvíli před dalším update
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

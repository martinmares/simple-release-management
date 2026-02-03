use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{Release, ImageMapping};

/// Request pro vytvoření nového release
#[derive(Debug, Deserialize)]
pub struct CreateReleaseRequest {
    pub bundle_version_id: Uuid,
    pub release_id: String,
    pub notes: Option<String>,
    pub created_by: Option<String>,
}

/// Request pro update release
#[derive(Debug, Deserialize)]
pub struct UpdateReleaseRequest {
    pub status: String,
    pub notes: Option<String>,
}

/// Response s release manifestem (seznam images s SHA)
#[derive(Debug, Serialize)]
pub struct ReleaseManifest {
    pub release_id: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub images: Vec<ManifestImage>,
}

/// Image v release manifestu
#[derive(Debug, Serialize)]
pub struct ManifestImage {
    pub source_image: String,
    pub source_tag: String,
    pub source_sha256: Option<String>,
    pub target_image: String,
    pub target_tag_template: Option<String>,
    pub target_sha256: Option<String>,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Vytvoří router pro releases endpoints
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/releases", get(list_all_releases))
        .route("/tenants/{tenant_id}/releases", get(list_releases).post(create_release))
        .route("/releases/{id}", get(get_release).put(update_release))
        .route("/releases/{id}/manifest", get(get_release_manifest))
        .with_state(pool)
}

/// GET /api/v1/releases - Seznam všech releases
async fn list_all_releases(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Release>>, (StatusCode, Json<ErrorResponse>)> {
    let releases = sqlx::query_as::<_, Release>(
        r#"
        SELECT r.*
        FROM releases r
        ORDER BY r.created_at DESC
        "#
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

    Ok(Json(releases))
}

/// GET /api/v1/tenants/{tenant_id}/releases - Seznam releases pro tenanta
async fn list_releases(
    State(pool): State<PgPool>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<Release>>, (StatusCode, Json<ErrorResponse>)> {
    let releases = sqlx::query_as::<_, Release>(
        r#"
        SELECT r.*
        FROM releases r
        JOIN bundle_versions bv ON bv.id = r.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        WHERE b.tenant_id = $1
        ORDER BY r.created_at DESC
        "#
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

    Ok(Json(releases))
}

/// GET /api/v1/releases/{id} - Detail release
async fn get_release(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Release>, (StatusCode, Json<ErrorResponse>)> {
    let release = sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = $1")
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

    match release {
        Some(release) => Ok(Json(release)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Release with id {} not found", id),
            }),
        )),
    }
}

/// POST /api/v1/tenants/{tenant_id}/releases - Vytvoření nového release
async fn create_release(
    State(pool): State<PgPool>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<CreateReleaseRequest>,
) -> Result<(StatusCode, Json<Release>), (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.release_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Release ID cannot be empty".to_string(),
            }),
        ));
    }

    // Zkontrolovat že bundle_version existuje a patří k tomuto tenantu
    let bundle_version_valid = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM bundle_versions bv
            JOIN bundles b ON b.id = bv.bundle_id
            WHERE bv.id = $1 AND b.tenant_id = $2
        )
        "#
    )
    .bind(payload.bundle_version_id)
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

    if !bundle_version_valid {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bundle version not found or doesn't belong to this tenant".to_string(),
            }),
        ));
    }

    // Zkontrolovat že všechny image mappings byly zkopírované (mají status success)
    let all_copied = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT NOT EXISTS(
            SELECT 1 FROM image_mappings
            WHERE bundle_version_id = $1 AND copy_status != 'success'
        )
        "#
    )
    .bind(payload.bundle_version_id)
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

    if !all_copied {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Cannot create release: not all images have been copied successfully".to_string(),
            }),
        ));
    }

    // Vytvoření release
    let release = sqlx::query_as::<_, Release>(
        "INSERT INTO releases (bundle_version_id, release_id, status, notes, created_by)
         VALUES ($1, $2, 'draft', $3, $4)
         RETURNING id, bundle_version_id, release_id, status, notes, created_by, created_at",
    )
    .bind(payload.bundle_version_id)
    .bind(&payload.release_id)
    .bind(&payload.notes)
    .bind(&payload.created_by)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("Release with ID '{}' already exists", payload.release_id),
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

    Ok((StatusCode::CREATED, Json(release)))
}

/// PUT /api/v1/releases/{id} - Update release
async fn update_release(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReleaseRequest>,
) -> Result<Json<Release>, (StatusCode, Json<ErrorResponse>)> {
    // Validace status
    let valid_statuses = ["draft", "released", "deployed"];
    if !valid_statuses.contains(&payload.status.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid status. Must be one of: {}", valid_statuses.join(", ")),
            }),
        ));
    }

    let release = sqlx::query_as::<_, Release>(
        "UPDATE releases
         SET status = $1, notes = $2
         WHERE id = $3
         RETURNING id, bundle_version_id, release_id, status, notes, created_by, created_at",
    )
    .bind(&payload.status)
    .bind(&payload.notes)
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

    match release {
        Some(release) => Ok(Json(release)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Release with id {} not found", id),
            }),
        )),
    }
}

/// GET /api/v1/releases/{id}/manifest - Release manifest s SHA pro deployment
async fn get_release_manifest(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReleaseManifest>, (StatusCode, Json<ErrorResponse>)> {
    // Získat release
    let release = sqlx::query_as::<_, Release>("SELECT * FROM releases WHERE id = $1")
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
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Release with id {} not found", id),
                }),
            )
        })?;

    // Získat všechny image mappings pro tento release
    let images = sqlx::query_as::<_, ImageMapping>(
        "SELECT * FROM image_mappings WHERE bundle_version_id = $1 ORDER BY created_at"
    )
    .bind(release.bundle_version_id)
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

    let manifest_images: Vec<ManifestImage> = images
        .into_iter()
        .map(|img| ManifestImage {
            source_image: img.source_image,
            source_tag: img.source_tag,
            source_sha256: img.source_sha256,
            target_image: img.target_image,
            target_tag_template: img.target_tag_template,
            target_sha256: img.target_sha256,
        })
        .collect();

    let manifest = ReleaseManifest {
        release_id: release.release_id,
        status: release.status,
        created_at: release.created_at,
        images: manifest_images,
    };

    Ok(Json(manifest))
}

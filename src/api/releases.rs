use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{db::models::Release, services::release_manifest::build_release_manifest};

/// Request pro vytvoření nového release
#[derive(Debug, Deserialize)]
pub struct CreateReleaseRequest {
    pub copy_job_id: Uuid,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReleaseSummary {
    pub id: Uuid,
    pub copy_job_id: Uuid,
    pub release_id: String,
    pub status: String,
    pub is_auto: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub bundle_id: Uuid,
    pub bundle_name: String,
    pub deploy_total: i64,
    pub deploy_success: i64,
    pub deploy_failed: i64,
    pub deploy_in_progress: i64,
    pub deploy_pending: i64,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Vytvoří router pro releases endpoints
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/releases", get(list_all_releases).post(create_release_global))
        .route("/tenants/{tenant_id}/releases", get(list_releases).post(create_release))
        .route("/releases/{id}", get(get_release).put(update_release))
        .route("/releases/{id}/manifest", get(get_release_manifest))
        .with_state(pool)
}

/// GET /api/v1/releases - Seznam všech releases
async fn list_all_releases(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<ReleaseSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let releases = sqlx::query_as::<_, ReleaseSummary>(
        r#"
        SELECT
            r.id,
            r.copy_job_id,
            r.release_id,
            r.status,
            r.is_auto,
            r.created_at,
            t.id AS tenant_id,
            t.name AS tenant_name,
            b.id AS bundle_id,
            b.name AS bundle_name,
            COALESCE(COUNT(dj.id), 0) AS deploy_total,
            COALESCE(SUM(CASE WHEN dj.status = 'success' THEN 1 ELSE 0 END), 0) AS deploy_success,
            COALESCE(SUM(CASE WHEN dj.status = 'failed' THEN 1 ELSE 0 END), 0) AS deploy_failed,
            COALESCE(SUM(CASE WHEN dj.status = 'in_progress' THEN 1 ELSE 0 END), 0) AS deploy_in_progress,
            COALESCE(SUM(CASE WHEN dj.status = 'pending' THEN 1 ELSE 0 END), 0) AS deploy_pending
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        JOIN tenants t ON t.id = b.tenant_id
        LEFT JOIN deploy_jobs dj ON dj.release_id = r.id
        GROUP BY r.id, t.id, b.id
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
) -> Result<Json<Vec<ReleaseSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let releases = sqlx::query_as::<_, ReleaseSummary>(
        r#"
        SELECT
            r.id,
            r.copy_job_id,
            r.release_id,
            r.status,
            r.is_auto,
            r.created_at,
            t.id AS tenant_id,
            t.name AS tenant_name,
            b.id AS bundle_id,
            b.name AS bundle_name,
            COALESCE(COUNT(dj.id), 0) AS deploy_total,
            COALESCE(SUM(CASE WHEN dj.status = 'success' THEN 1 ELSE 0 END), 0) AS deploy_success,
            COALESCE(SUM(CASE WHEN dj.status = 'failed' THEN 1 ELSE 0 END), 0) AS deploy_failed,
            COALESCE(SUM(CASE WHEN dj.status = 'in_progress' THEN 1 ELSE 0 END), 0) AS deploy_in_progress,
            COALESCE(SUM(CASE WHEN dj.status = 'pending' THEN 1 ELSE 0 END), 0) AS deploy_pending
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        JOIN tenants t ON t.id = b.tenant_id
        LEFT JOIN deploy_jobs dj ON dj.release_id = r.id
        WHERE b.tenant_id = $1
        GROUP BY r.id, t.id, b.id
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
    let release_id = payload.release_id.trim().to_string();
    if release_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Release ID cannot be empty".to_string(),
            }),
        ));
    }

    // Zkontrolovat že copy_job existuje a patří k tomuto tenantu
    let copy_job_valid = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM copy_jobs cj
            JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
            JOIN bundles b ON b.id = bv.bundle_id
            WHERE cj.id = $1 AND b.tenant_id = $2
        )
        "#
    )
    .bind(payload.copy_job_id)
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

    if !copy_job_valid {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Copy job not found or doesn't belong to this tenant".to_string(),
            }),
        ));
    }

    // Zkontrolovat že copy job je úspěšný
    let job_success = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT status = 'success' FROM copy_jobs WHERE id = $1
        "#
    )
    .bind(payload.copy_job_id)
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

    if !job_success {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Cannot create release: copy job is not successful".to_string(),
            }),
        ));
    }

    // Vytvoření release
    let release = sqlx::query_as::<_, Release>(
        "INSERT INTO releases (copy_job_id, release_id, status, notes, created_by, is_auto)
         VALUES ($1, $2, 'draft', $3, $4, false)
         RETURNING id, copy_job_id, release_id, status, notes, created_by, is_auto, auto_reason, created_at",
    )
    .bind(payload.copy_job_id)
    .bind(&release_id)
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
                        error: format!("Release with ID '{}' already exists", release_id),
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

/// POST /api/v1/releases - Vytvoření nového release bez tenanta (tenant se odvodí z copy jobu)
async fn create_release_global(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateReleaseRequest>,
) -> Result<(StatusCode, Json<Release>), (StatusCode, Json<ErrorResponse>)> {
    let release_id = payload.release_id.trim().to_string();
    if release_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Release ID cannot be empty".to_string(),
            }),
        ));
    }

    // Zkontrolovat že copy_job existuje
    let copy_job_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM copy_jobs WHERE id = $1)"
    )
    .bind(payload.copy_job_id)
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

    if !copy_job_exists {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Copy job not found".to_string(),
            }),
        ));
    }

    // Zkontrolovat že copy job je úspěšný
    let job_success = sqlx::query_scalar::<_, bool>(
        "SELECT status = 'success' FROM copy_jobs WHERE id = $1"
    )
    .bind(payload.copy_job_id)
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

    if !job_success {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Cannot create release: copy job is not successful".to_string(),
            }),
        ));
    }

    let release = sqlx::query_as::<_, Release>(
        "INSERT INTO releases (copy_job_id, release_id, status, notes, created_by, is_auto)
         VALUES ($1, $2, 'draft', $3, $4, false)
         RETURNING id, copy_job_id, release_id, status, notes, created_by, is_auto, auto_reason, created_at",
    )
    .bind(payload.copy_job_id)
    .bind(&release_id)
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
                        error: format!("Release with ID '{}' already exists", release_id),
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
         RETURNING id, copy_job_id, release_id, status, notes, created_by, is_auto, auto_reason, created_at",
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

/// GET /api/v1/releases/{id}/manifest - Release manifest (YAML) pro deployment
async fn get_release_manifest(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let manifest = build_release_manifest(&pool, id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to build manifest: {}", e),
            }),
        )
    })?;

    let yaml = serde_yaml_ng::to_string(&manifest).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to serialize manifest: {}", e),
            }),
        )
    })?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/yaml; charset=utf-8")],
        yaml,
    ))
}

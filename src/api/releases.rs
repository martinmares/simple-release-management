use axum::{
    extract::{Path, State, Query},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{auth::AuthContext, db::models::Release, services::release_manifest::build_release_manifest};

/// Request pro vytvoření nového release
#[derive(Debug, Deserialize)]
pub struct CreateReleaseRequest {
    pub copy_job_id: Uuid,
    pub release_id: String,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub source_ref_mode: Option<String>,
}

/// Request pro update release
#[derive(Debug, Deserialize)]
pub struct UpdateReleaseRequest {
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompareReleasesQuery {
    pub release_a: Uuid,
    pub release_b: Uuid,
}

#[derive(Debug, Serialize)]
pub struct CompareReleaseRow {
    pub app_name: String,
    pub container_name: String,
    pub digest_a: Option<String>,
    pub digest_b: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReleaseSummary {
    pub id: Uuid,
    pub copy_job_id: Uuid,
    pub release_id: String,
    pub status: String,
    pub source_ref_mode: String,
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
    pub environment_id: Option<Uuid>,
    pub environment_name: Option<String>,
    pub environment_color: Option<String>,
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
        .route("/releases/compare", get(compare_releases))
        .route("/releases/{id}", get(get_release).put(update_release))
        .route("/releases/{id}/manifest", get(get_release_manifest))
        .with_state(pool)
}

/// GET /api/v1/releases - Seznam všech releases
async fn list_all_releases(
    Extension(auth): Extension<AuthContext>,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<ReleaseSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let releases = if auth.is_admin() {
        sqlx::query_as::<_, ReleaseSummary>(
            r#"
            SELECT
                r.id,
                r.copy_job_id,
                r.release_id,
                r.status,
                r.source_ref_mode,
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
                COALESCE(SUM(CASE WHEN dj.status = 'pending' THEN 1 ELSE 0 END), 0) AS deploy_pending,
                e.id AS environment_id,
                e.name AS environment_name,
                e.color AS environment_color
            FROM releases r
            JOIN copy_jobs cj ON cj.id = r.copy_job_id
            JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
            JOIN bundles b ON b.id = bv.bundle_id
            JOIN tenants t ON t.id = b.tenant_id
            LEFT JOIN environments e ON e.id = cj.environment_id
            LEFT JOIN deploy_jobs dj ON dj.release_id = r.id
            GROUP BY r.id, t.id, b.id, e.id
            ORDER BY r.created_at DESC
            "#,
        )
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, ReleaseSummary>(
            r#"
            SELECT
                r.id,
                r.copy_job_id,
                r.release_id,
                r.status,
                r.source_ref_mode,
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
                COALESCE(SUM(CASE WHEN dj.status = 'pending' THEN 1 ELSE 0 END), 0) AS deploy_pending,
                e.id AS environment_id,
                e.name AS environment_name,
                e.color AS environment_color
            FROM releases r
            JOIN copy_jobs cj ON cj.id = r.copy_job_id
            JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
            JOIN bundles b ON b.id = bv.bundle_id
            JOIN tenants t ON t.id = b.tenant_id
            LEFT JOIN environments e ON e.id = cj.environment_id
            LEFT JOIN deploy_jobs dj ON dj.release_id = r.id
            WHERE t.id = ANY($1)
            GROUP BY r.id, t.id, b.id, e.id
            ORDER BY r.created_at DESC
            "#,
        )
        .bind(&auth.tenant_ids)
        .fetch_all(&pool)
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
            r.source_ref_mode,
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
            COALESCE(SUM(CASE WHEN dj.status = 'pending' THEN 1 ELSE 0 END), 0) AS deploy_pending,
            e.id AS environment_id,
            e.name AS environment_name,
            e.color AS environment_color
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        JOIN bundles b ON b.id = bv.bundle_id
        JOIN tenants t ON t.id = b.tenant_id
        LEFT JOIN environments e ON e.id = cj.environment_id
        LEFT JOIN deploy_jobs dj ON dj.release_id = r.id
        WHERE b.tenant_id = $1
        GROUP BY r.id, t.id, b.id, e.id
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

/// GET /api/v1/releases/compare?release_a=...&release_b=... - porovnání digestů mezi dvěma releases
async fn compare_releases(
    Extension(auth): Extension<AuthContext>,
    State(pool): State<PgPool>,
    Query(params): Query<CompareReleasesQuery>,
) -> Result<Json<Vec<CompareReleaseRow>>, (StatusCode, Json<ErrorResponse>)> {
    if params.release_a == params.release_b {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Select two different releases".to_string(),
            }),
        ));
    }

    if !auth.is_admin() {
        let tenant_a = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT b.tenant_id
            FROM releases r
            JOIN copy_jobs cj ON cj.id = r.copy_job_id
            JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
            JOIN bundles b ON b.id = bv.bundle_id
            WHERE r.id = $1
            "#
        )
        .bind(params.release_a)
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

        let tenant_b = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT b.tenant_id
            FROM releases r
            JOIN copy_jobs cj ON cj.id = r.copy_job_id
            JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
            JOIN bundles b ON b.id = bv.bundle_id
            WHERE r.id = $1
            "#
        )
        .bind(params.release_b)
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

        if let Some(tenant_id) = tenant_a {
            if !auth.is_tenant_allowed(tenant_id) {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "Tenant access denied".to_string(),
                    }),
                ));
            }
        }
        if let Some(tenant_id) = tenant_b {
            if !auth.is_tenant_allowed(tenant_id) {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "Tenant access denied".to_string(),
                    }),
                ));
            }
        }
    }

    let manifest_a = build_release_manifest(&pool, params.release_a).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to build manifest A: {}", e),
            }),
        )
    })?;
    let manifest_b = build_release_manifest(&pool, params.release_b).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to build manifest B: {}", e),
            }),
        )
    })?;

    let mut map_a: HashMap<(String, String), Option<String>> = HashMap::new();
    for img in manifest_a.images {
        let key = (
            img.app_name,
            img.container_name.unwrap_or_default(),
        );
        map_a.insert(key, img.digest);
    }

    let mut map_b: HashMap<(String, String), Option<String>> = HashMap::new();
    for img in manifest_b.images {
        let key = (
            img.app_name,
            img.container_name.unwrap_or_default(),
        );
        map_b.insert(key, img.digest);
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

        results.push(CompareReleaseRow {
            app_name,
            container_name,
            digest_a,
            digest_b,
            status,
        });
    }

    Ok(Json(results))
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

    let source_ref_mode = payload
        .source_ref_mode
        .unwrap_or_else(|| "tag".to_string())
        .to_lowercase();
    let source_ref_mode = match source_ref_mode.as_str() {
        "digest" => "digest".to_string(),
        _ => "tag".to_string(),
    };

    // Vytvoření release
    let release = sqlx::query_as::<_, Release>(
        "INSERT INTO releases (copy_job_id, release_id, status, source_ref_mode, notes, created_by, is_auto)
         VALUES ($1, $2, 'draft', $3, $4, $5, false)
         RETURNING id, copy_job_id, release_id, status, source_ref_mode, notes, created_by, is_auto, auto_reason, created_at",
    )
    .bind(payload.copy_job_id)
    .bind(&release_id)
    .bind(&source_ref_mode)
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
    Extension(auth): Extension<AuthContext>,
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

    let tenant_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT b.tenant_id
         FROM copy_jobs cj
         JOIN bundle_versions bv ON cj.bundle_version_id = bv.id
         JOIN bundles b ON bv.bundle_id = b.id
         WHERE cj.id = $1"
    )
    .bind(payload.copy_job_id)
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

    if let Some(tenant_id) = tenant_id {
        if !auth.is_tenant_allowed(tenant_id) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Tenant access denied".to_string(),
                }),
            ));
        }
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

    let source_ref_mode = payload
        .source_ref_mode
        .unwrap_or_else(|| "tag".to_string())
        .to_lowercase();
    let source_ref_mode = match source_ref_mode.as_str() {
        "digest" => "digest".to_string(),
        _ => "tag".to_string(),
    };

    let release = sqlx::query_as::<_, Release>(
        "INSERT INTO releases (copy_job_id, release_id, status, source_ref_mode, notes, created_by, is_auto)
         VALUES ($1, $2, 'draft', $3, $4, $5, false)
         RETURNING id, copy_job_id, release_id, status, source_ref_mode, notes, created_by, is_auto, auto_reason, created_at",
    )
    .bind(payload.copy_job_id)
    .bind(&release_id)
    .bind(&source_ref_mode)
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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthContext;
use crate::db::models::{Bundle, BundleVersion, ImageMapping};

/// Request pro vytvoření nového bundle
#[derive(Debug, Deserialize)]
pub struct CreateBundleRequest {
    pub source_registry_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub auto_tag_enabled: Option<bool>,
}

/// Request pro update bundle
#[derive(Debug, Deserialize)]
pub struct UpdateBundleRequest {
    pub name: String,
    pub description: Option<String>,
    pub source_registry_id: Uuid,
    pub auto_tag_enabled: Option<bool>,
}

/// Request pro vytvoření nové verze bundle
#[derive(Debug, Deserialize)]
pub struct CreateBundleVersionRequest {
    pub change_note: Option<String>,
}

/// BundleVersion s počtem images
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BundleVersionWithCount {
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub version: i32,
    pub change_note: Option<String>,
    pub created_by: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub image_count: i32,
}

/// Copy job summary pro bundle
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BundleCopyJobSummary {
    pub job_id: Uuid,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BundleReleaseSummary {
    pub id: Uuid,
    pub release_id: String,
    pub status: String,
    pub is_auto: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BundleDeployJobSummary {
    pub id: Uuid,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub commit_sha: Option<String>,
    pub tag_name: Option<String>,
    pub dry_run: bool,
    pub target_name: String,
    pub env_name: String,
    pub release_db_id: Uuid,
    pub release_id: String,
    pub is_auto: bool,
}

/// Request pro přidání image mapping
#[derive(Debug, Deserialize)]
pub struct CreateImageMappingRequest {
    pub source_image: String,
    pub source_tag: String,
    pub target_image: String,
    pub app_name: String,
    pub container_name: Option<String>,
}

/// Response s chybou
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Request pro archivaci verze
#[derive(Debug, Deserialize)]
pub struct ArchiveBundleVersionRequest {
    pub is_archived: bool,
}

/// Response s bundle včetně počtu image mappings
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BundleWithStats {
    // Bundle fields
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_registry_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub auto_tag_enabled: bool,
    pub current_version: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // Stats
    pub image_count: i64,
}

/// Vytvoří router pro bundles endpoints
pub fn router(pool: PgPool) -> Router {
    Router::new()
        // Bundle CRUD
        .route("/bundles", get(list_all_bundles))
        .route("/tenants/{tenant_id}/bundles", get(list_bundles).post(create_bundle))
        .route("/bundles/{id}", get(get_bundle).put(update_bundle).delete(delete_bundle))

        // Bundle versions
        .route("/bundles/{bundle_id}/versions", get(list_bundle_versions).post(create_bundle_version))
        .route("/bundles/{bundle_id}/versions/{version}", get(get_bundle_version))
        .route("/bundles/{bundle_id}/versions/{version}/archive", put(set_bundle_version_archive))
        .route("/bundles/{bundle_id}/copy-jobs", get(list_bundle_copy_jobs))
        .route("/bundles/{bundle_id}/releases", get(list_bundle_releases))
        .route("/bundles/{bundle_id}/deployments", get(list_bundle_deployments))

        // Image mappings
        .route("/bundles/{bundle_id}/versions/{version}/images", get(list_image_mappings).post(create_image_mapping))
        .route("/bundles/{bundle_id}/versions/{version}/images/{mapping_id}", get(get_image_mapping).delete(delete_image_mapping))

        .with_state(pool)
}

/// GET /api/v1/bundles - Seznam všech bundles
async fn list_all_bundles(
    Extension(auth): Extension<AuthContext>,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<BundleWithStats>>, (StatusCode, Json<ErrorResponse>)> {
    let bundles = if auth.is_admin() {
        sqlx::query_as::<_, BundleWithStats>(
            r#"
            SELECT
                b.*,
                COALESCE(
                    (SELECT COUNT(*)
                     FROM image_mappings im
                     WHERE im.bundle_version_id = (
                         SELECT bv.id
                         FROM bundle_versions bv
                         WHERE bv.bundle_id = b.id
                         ORDER BY bv.version DESC
                         LIMIT 1
                     )),
                    0
                ) as image_count
            FROM bundles b
            ORDER BY b.created_at DESC
            "#,
        )
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, BundleWithStats>(
            r#"
            SELECT
                b.*,
                COALESCE(
                    (SELECT COUNT(*)
                     FROM image_mappings im
                     WHERE im.bundle_version_id = (
                         SELECT bv.id
                         FROM bundle_versions bv
                         WHERE bv.bundle_id = b.id
                         ORDER BY bv.version DESC
                         LIMIT 1
                     )),
                    0
                ) as image_count
            FROM bundles b
            WHERE b.tenant_id = ANY($1)
            ORDER BY b.created_at DESC
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

    Ok(Json(bundles))
}

/// GET /api/v1/tenants/{tenant_id}/bundles - Seznam bundles pro tenanta
async fn list_bundles(
    State(pool): State<PgPool>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<BundleWithStats>>, (StatusCode, Json<ErrorResponse>)> {
    let bundles = sqlx::query_as::<_, BundleWithStats>(
        r#"
        SELECT
            b.*,
            COALESCE(
                (SELECT COUNT(*)
                 FROM image_mappings im
                 WHERE im.bundle_version_id = (
                     SELECT bv.id
                     FROM bundle_versions bv
                     WHERE bv.bundle_id = b.id
                     ORDER BY bv.version DESC
                     LIMIT 1
                 )),
                0
            ) as image_count
        FROM bundles b
        WHERE b.tenant_id = $1
        ORDER BY b.created_at DESC
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

    Ok(Json(bundles))
}

/// GET /api/v1/bundles/{id} - Detail bundle
async fn get_bundle(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Bundle>, (StatusCode, Json<ErrorResponse>)> {
    let bundle = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles WHERE id = $1")
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

    match bundle {
        Some(bundle) => Ok(Json(bundle)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Bundle with id {} not found", id),
            }),
        )),
    }
}

/// POST /api/v1/tenants/{tenant_id}/bundles - Vytvoření nového bundle
async fn create_bundle(
    State(pool): State<PgPool>,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<CreateBundleRequest>,
) -> Result<(StatusCode, Json<Bundle>), (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bundle name cannot be empty".to_string(),
            }),
        ));
    }

    // Zkontrolovat že tenant existuje
    let tenant_exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM tenants WHERE id = $1)")
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

    if !tenant_exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Tenant with id {} not found", tenant_id),
            }),
        ));
    }

    // Zkontrolovat že source registry existuje a patří k tomuto tenantu
    let registry_valid = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM registries
            WHERE id = $1 AND tenant_id = $2
        )"
    )
    .bind(payload.source_registry_id)
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

    if !registry_valid {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Source registry not found or doesn't belong to this tenant".to_string(),
            }),
        ));
    }

    // Začít transakci
    let mut tx = pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    // Vytvoření bundle
    let auto_tag_enabled = payload.auto_tag_enabled.unwrap_or(false);

    let bundle = sqlx::query_as::<_, Bundle>(
        "INSERT INTO bundles (tenant_id, source_registry_id, name, description, auto_tag_enabled, current_version)
         VALUES ($1, $2, $3, $4, $5, 1)
         RETURNING id, tenant_id, source_registry_id, name, description, auto_tag_enabled, current_version, created_at",
    )
    .bind(tenant_id)
    .bind(payload.source_registry_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(auto_tag_enabled)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("Bundle with name '{}' already exists in this tenant", payload.name),
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

    // Vytvoření první verze
    sqlx::query(
        "INSERT INTO bundle_versions (bundle_id, version, change_note, created_by)
         VALUES ($1, 1, $2, $3)"
    )
    .bind(bundle.id)
    .bind("Initial version")
    .bind("system")
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create initial version: {}", e),
            }),
        )
    })?;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to commit transaction: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(bundle)))
}

/// PUT /api/v1/bundles/{id} - Update bundle
async fn update_bundle(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateBundleRequest>,
) -> Result<Json<Bundle>, (StatusCode, Json<ErrorResponse>)> {
    if payload.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bundle name cannot be empty".to_string(),
            }),
        ));
    }

    let auto_tag_enabled = payload.auto_tag_enabled.unwrap_or(false);

    let bundle = sqlx::query_as::<_, Bundle>(
        "UPDATE bundles
         SET name = $1, description = $2, source_registry_id = $3, auto_tag_enabled = $4
         WHERE id = $5
         RETURNING id, tenant_id, source_registry_id, name, description, auto_tag_enabled, current_version, created_at",
    )
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(payload.source_registry_id)
    .bind(auto_tag_enabled)
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!("Bundle with name '{}' already exists in this tenant", payload.name),
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

    match bundle {
        Some(bundle) => Ok(Json(bundle)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Bundle with id {} not found", id),
            }),
        )),
    }
}

/// DELETE /api/v1/bundles/{id} - Smazání bundle
async fn delete_bundle(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let result = sqlx::query("DELETE FROM bundles WHERE id = $1")
        .bind(id)
        .execute(&pool)
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
                error: format!("Bundle with id {} not found", id),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/bundles/{bundle_id}/versions - Seznam verzí bundle
async fn list_bundle_versions(
    State(pool): State<PgPool>,
    Path(bundle_id): Path<Uuid>,
) -> Result<Json<Vec<BundleVersionWithCount>>, (StatusCode, Json<ErrorResponse>)> {
    let versions = sqlx::query_as::<_, BundleVersionWithCount>(
        "SELECT bv.*, COUNT(im.id)::int as image_count
         FROM bundle_versions bv
         LEFT JOIN image_mappings im ON im.bundle_version_id = bv.id
         WHERE bv.bundle_id = $1
         GROUP BY bv.id
         ORDER BY bv.version DESC"
    )
    .bind(bundle_id)
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

    Ok(Json(versions))
}

/// GET /api/v1/bundles/{bundle_id}/versions/{version} - Detail verze
async fn get_bundle_version(
    State(pool): State<PgPool>,
    Path((bundle_id, version)): Path<(Uuid, i32)>,
) -> Result<Json<BundleVersion>, (StatusCode, Json<ErrorResponse>)> {
    let bundle_version = sqlx::query_as::<_, BundleVersion>(
        "SELECT * FROM bundle_versions WHERE bundle_id = $1 AND version = $2"
    )
    .bind(bundle_id)
    .bind(version)
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

    match bundle_version {
        Some(version) => Ok(Json(version)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Bundle version {} not found for bundle {}", version, bundle_id),
            }),
        )),
    }
}

/// PUT /api/v1/bundles/{bundle_id}/versions/{version}/archive
async fn set_bundle_version_archive(
    State(pool): State<PgPool>,
    Path((bundle_id, version)): Path<(Uuid, i32)>,
    Json(payload): Json<ArchiveBundleVersionRequest>,
) -> Result<Json<BundleVersion>, (StatusCode, Json<ErrorResponse>)> {
    let updated = sqlx::query_as::<_, BundleVersion>(
        "UPDATE bundle_versions
         SET is_archived = $1
         WHERE bundle_id = $2 AND version = $3
         RETURNING id, bundle_id, version, change_note, created_by, is_archived, created_at"
    )
    .bind(payload.is_archived)
    .bind(bundle_id)
    .bind(version)
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

    match updated {
        Some(version) => Ok(Json(version)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Bundle version {} not found", version),
            }),
        )),
    }
}

/// GET /api/v1/bundles/{bundle_id}/copy-jobs - Seznam copy jobů pro bundle
async fn list_bundle_copy_jobs(
    State(pool): State<PgPool>,
    Path(bundle_id): Path<Uuid>,
) -> Result<Json<Vec<BundleCopyJobSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let jobs = sqlx::query_as::<_, BundleCopyJobSummary>(
        r#"
        SELECT
            cj.id AS job_id,
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
        WHERE bv.bundle_id = $1
        ORDER BY cj.started_at DESC
        LIMIT 50
        "#
    )
    .bind(bundle_id)
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

    Ok(Json(jobs))
}

/// GET /api/v1/bundles/{bundle_id}/releases - Seznam release pro bundle
async fn list_bundle_releases(
    State(pool): State<PgPool>,
    Path(bundle_id): Path<Uuid>,
) -> Result<Json<Vec<BundleReleaseSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let releases = sqlx::query_as::<_, BundleReleaseSummary>(
        r#"
        SELECT r.id, r.release_id, r.status, r.is_auto, r.created_at
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        WHERE bv.bundle_id = $1
        ORDER BY r.created_at DESC
        LIMIT 50
        "#
    )
    .bind(bundle_id)
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

/// GET /api/v1/bundles/{bundle_id}/deployments - Seznam deploy jobů pro bundle
async fn list_bundle_deployments(
    State(pool): State<PgPool>,
    Path(bundle_id): Path<Uuid>,
) -> Result<Json<Vec<BundleDeployJobSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let jobs = sqlx::query_as::<_, BundleDeployJobSummary>(
        r#"
        SELECT
            dj.id,
            dj.status,
            dj.started_at,
            dj.completed_at,
            dj.commit_sha,
            dj.tag_name,
            dj.dry_run,
            e.name as target_name,
            e.slug AS env_name,
            r.id as release_db_id,
            r.release_id,
            r.is_auto
        FROM deploy_jobs dj
        JOIN environments e ON e.id = dj.environment_id
        JOIN releases r ON r.id = dj.release_id
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        JOIN bundle_versions bv ON bv.id = cj.bundle_version_id
        WHERE bv.bundle_id = $1
        ORDER BY dj.started_at DESC
        LIMIT 50
        "#
    )
    .bind(bundle_id)
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

    Ok(Json(jobs))
}

/// POST /api/v1/bundles/{bundle_id}/versions - Vytvoření nové verze
async fn create_bundle_version(
    State(pool): State<PgPool>,
    Path(bundle_id): Path<Uuid>,
    Json(payload): Json<CreateBundleVersionRequest>,
) -> Result<(StatusCode, Json<BundleVersion>), (StatusCode, Json<ErrorResponse>)> {
    let mut tx = pool.begin().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    // Získat aktuální verzi a inkrementovat
    let current_version: i32 = sqlx::query_scalar(
        "SELECT current_version FROM bundles WHERE id = $1 FOR UPDATE"
    )
    .bind(bundle_id)
    .fetch_optional(&mut *tx)
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

    let new_version = current_version + 1;

    // Vytvořit novou verzi
    let bundle_version = sqlx::query_as::<_, BundleVersion>(
        "INSERT INTO bundle_versions (bundle_id, version, change_note)
         VALUES ($1, $2, $3)
         RETURNING id, bundle_id, version, change_note, created_by, is_archived, created_at"
    )
    .bind(bundle_id)
    .bind(new_version)
    .bind(&payload.change_note)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    // Archivovat předchozí verze
    sqlx::query(
        "UPDATE bundle_versions SET is_archived = TRUE WHERE bundle_id = $1 AND version < $2"
    )
    .bind(bundle_id)
    .bind(new_version)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to archive previous versions: {}", e),
            }),
        )
    })?;

    // Aktualizovat current_version v bundle
    sqlx::query("UPDATE bundles SET current_version = $1 WHERE id = $2")
        .bind(new_version)
        .bind(bundle_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to update current_version: {}", e),
                }),
            )
        })?;

    tx.commit().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to commit transaction: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(bundle_version)))
}

/// GET /api/v1/bundles/{bundle_id}/versions/{version}/images - Seznam image mappings
async fn list_image_mappings(
    State(pool): State<PgPool>,
    Path((bundle_id, version)): Path<(Uuid, i32)>,
) -> Result<Json<Vec<ImageMapping>>, (StatusCode, Json<ErrorResponse>)> {
    let mappings = sqlx::query_as::<_, ImageMapping>(
        r#"
        SELECT im.*
        FROM image_mappings im
        JOIN bundle_versions bv ON bv.id = im.bundle_version_id
        WHERE bv.bundle_id = $1 AND bv.version = $2
        ORDER BY im.created_at
        "#
    )
    .bind(bundle_id)
    .bind(version)
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

    Ok(Json(mappings))
}

/// GET /api/v1/bundles/{bundle_id}/versions/{version}/images/{mapping_id} - Detail image mapping
async fn get_image_mapping(
    State(pool): State<PgPool>,
    Path((_bundle_id, _version, mapping_id)): Path<(Uuid, i32, Uuid)>,
) -> Result<Json<ImageMapping>, (StatusCode, Json<ErrorResponse>)> {
    let mapping = sqlx::query_as::<_, ImageMapping>(
        "SELECT * FROM image_mappings WHERE id = $1"
    )
    .bind(mapping_id)
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

    match mapping {
        Some(mapping) => Ok(Json(mapping)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Image mapping with id {} not found", mapping_id),
            }),
        )),
    }
}

/// POST /api/v1/bundles/{bundle_id}/versions/{version}/images - Přidání image mapping
async fn create_image_mapping(
    State(pool): State<PgPool>,
    Path((bundle_id, version)): Path<(Uuid, i32)>,
    Json(payload): Json<CreateImageMappingRequest>,
) -> Result<(StatusCode, Json<ImageMapping>), (StatusCode, Json<ErrorResponse>)> {
    // Validace
    if payload.source_image.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Source image cannot be empty".to_string(),
            }),
        ));
    }
    if payload.app_name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "App name cannot be empty".to_string(),
            }),
        ));
    }

    // Získat bundle_version_id
    let bundle_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM bundle_versions WHERE bundle_id = $1 AND version = $2"
    )
    .bind(bundle_id)
    .bind(version)
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
                error: format!("Bundle version {} not found for bundle {}", version, bundle_id),
            }),
        )
    })?;

    // Vytvořit image mapping
    let source_tag = if payload.source_tag.trim().is_empty() {
        "latest".to_string()
    } else {
        payload.source_tag.clone()
    };
    let container_name = payload
        .container_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);

    let mapping = sqlx::query_as::<_, ImageMapping>(
        "INSERT INTO image_mappings
         (bundle_version_id, source_image, source_tag, target_image, app_name, container_name)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *"
    )
    .bind(bundle_version_id)
    .bind(&payload.source_image)
    .bind(&source_tag)
    .bind(&payload.target_image)
    .bind(&payload.app_name)
    .bind(&container_name)
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

    Ok((StatusCode::CREATED, Json(mapping)))
}

/// DELETE /api/v1/bundles/{bundle_id}/versions/{version}/images/{mapping_id} - Smazání image mapping
async fn delete_image_mapping(
    State(pool): State<PgPool>,
    Path((_bundle_id, _version, mapping_id)): Path<(Uuid, i32, Uuid)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let _ = pool;
    let _ = mapping_id;
    Err((
        StatusCode::METHOD_NOT_ALLOWED,
        Json(ErrorResponse {
            error: "Image mappings are immutable and cannot be deleted".to_string(),
        }),
    ))
}

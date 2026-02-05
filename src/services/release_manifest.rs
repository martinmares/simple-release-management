use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ReleaseManifest {
    pub release_id: String,
    pub created_at: DateTime<Utc>,
    pub images: Vec<ReleaseManifestImage>,
}

#[derive(Debug, Serialize)]
pub struct ReleaseManifestImage {
    pub app_name: String,
    pub container_name: Option<String>,
    pub image: String,
    pub tag: String,
    pub digest: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ReleaseBaseRow {
    release_id: String,
    created_at: DateTime<Utc>,
    copy_job_id: Uuid,
    target_registry_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct ManifestImageRow {
    target_image: String,
    target_tag: String,
    target_sha256: Option<String>,
    app_name: String,
    container_name: Option<String>,
}

pub async fn build_release_manifest(pool: &PgPool, release_db_id: Uuid) -> Result<ReleaseManifest> {
    let base = sqlx::query_as::<_, ReleaseBaseRow>(
        r#"
        SELECT r.release_id, r.created_at, r.copy_job_id, cj.target_registry_id
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        WHERE r.id = $1
        "#,
    )
    .bind(release_db_id)
    .fetch_one(pool)
    .await?;

    let registry_base_url = if let Some(registry_id) = base.target_registry_id {
        sqlx::query_scalar::<_, String>("SELECT base_url FROM registries WHERE id = $1")
            .bind(registry_id)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };

    let images = sqlx::query_as::<_, ManifestImageRow>(
        r#"
        SELECT cji.target_image, cji.target_tag, cji.target_sha256, im.app_name, im.container_name
        FROM copy_job_images cji
        JOIN image_mappings im ON im.id = cji.image_mapping_id
        WHERE cji.copy_job_id = $1
        ORDER BY cji.created_at
        "#,
    )
    .bind(base.copy_job_id)
    .fetch_all(pool)
    .await?;

    let images = images
        .into_iter()
        .map(|img| {
            let base_image = if let Some(base_url) = registry_base_url.as_ref() {
                let base = normalize_registry_base(base_url);
                format!("{}/{}", base, img.target_image)
            } else {
                img.target_image
            };
            ReleaseManifestImage {
                app_name: img.app_name,
                container_name: img.container_name,
                image: base_image,
                tag: img.target_tag,
                digest: img.target_sha256,
            }
        })
        .collect();

    Ok(ReleaseManifest {
        release_id: base.release_id,
        created_at: base.created_at,
        images,
    })
}

fn normalize_registry_base(base_url: &str) -> String {
    let trimmed = base_url.trim();
    let without_scheme = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    without_scheme.trim_end_matches('/').to_string()
}

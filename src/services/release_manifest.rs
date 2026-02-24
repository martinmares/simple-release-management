use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ReleaseManifest {
    pub release_id: String,
    pub created_at: DateTime<Utc>,
    pub registry_base: Option<String>,
    pub images: Vec<ReleaseManifestImage>,
    pub extra_tags: Vec<String>,
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
    environment_id: Option<Uuid>,
    extra_tags: Option<Vec<String>>,
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
        SELECT r.release_id, r.created_at, r.copy_job_id, cj.target_registry_id, cj.environment_id, r.extra_tags
        FROM releases r
        JOIN copy_jobs cj ON cj.id = r.copy_job_id
        WHERE r.id = $1
        "#,
    )
    .bind(release_db_id)
    .fetch_one(pool)
    .await?;

    let registry_base = if let Some(registry_id) = base.target_registry_id {
        let base_url = sqlx::query_scalar::<_, String>("SELECT base_url FROM registries WHERE id = $1")
            .bind(registry_id)
            .fetch_optional(pool)
            .await?;

        let project_path_override = if let Some(env_id) = base.environment_id {
            sqlx::query_scalar::<_, String>(
                "SELECT project_path_override FROM environment_registry_paths WHERE environment_id = $1 AND registry_id = $2 AND role = 'target'",
            )
            .bind(env_id)
            .bind(registry_id)
            .fetch_optional(pool)
            .await?
        } else {
            None
        };

        let default_project_path = sqlx::query_scalar::<_, String>(
            "SELECT default_project_path FROM registries WHERE id = $1",
        )
        .bind(registry_id)
        .fetch_optional(pool)
        .await?;

        let project_path = project_path_override
            .or(default_project_path)
            .and_then(|p| normalize_project_path(&p));

        base_url.map(|url| join_registry_base(&url, project_path.as_deref()))
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
            let base_image = if let Some(base) = registry_base.as_ref() {
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
        registry_base,
        images,
        extra_tags: base.extra_tags.unwrap_or_default(),
    })
}

fn normalize_registry_base(base_url: &str) -> String {
    let trimmed = base_url.trim();
    let without_scheme = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    without_scheme.trim_end_matches('/').to_string()
}

fn normalize_project_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn join_registry_base(base_url: &str, project_path: Option<&str>) -> String {
    let base = normalize_registry_base(base_url);
    match project_path {
        Some(p) if !p.is_empty() => format!("{}/{}", base, p.trim_matches('/')),
        _ => base,
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Tenant - základní organizační jednotka
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Role registry (source/target/both)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistryRole {
    Source,
    Target,
    Both,
}

impl std::fmt::Display for RegistryRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryRole::Source => write!(f, "source"),
            RegistryRole::Target => write!(f, "target"),
            RegistryRole::Both => write!(f, "both"),
        }
    }
}

/// Registry - Docker/Harbor/Quay registry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Registry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub registry_type: String,
    pub base_url: String,
    pub credentials_path: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Bundle - mapování images ze source do target
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Bundle {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_registry_id: Uuid,
    pub target_registry_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub current_version: i32,
    pub created_at: DateTime<Utc>,
}

/// Bundle Version - verzování bundle
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BundleVersion {
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub version: i32,
    pub change_note: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Copy status pro image mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyStatus {
    Pending,
    InProgress,
    Success,
    Failed,
}

impl std::fmt::Display for CopyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopyStatus::Pending => write!(f, "pending"),
            CopyStatus::InProgress => write!(f, "in_progress"),
            CopyStatus::Success => write!(f, "success"),
            CopyStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Image Mapping - jednotlivé mapování image
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ImageMapping {
    pub id: Uuid,
    pub bundle_version_id: Uuid,

    // Source
    pub source_image: String,
    pub source_tag: String,
    pub source_sha256: String,

    // Target
    pub target_image: String,
    pub target_tag_template: String,
    pub target_sha256: Option<String>,

    // Status
    pub copy_status: String,
    pub copied_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,

    pub created_at: DateTime<Utc>,
}

/// Release status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseStatus {
    Draft,
    Released,
    Deployed,
}

impl std::fmt::Display for ReleaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleaseStatus::Draft => write!(f, "draft"),
            ReleaseStatus::Released => write!(f, "released"),
            ReleaseStatus::Deployed => write!(f, "deployed"),
        }
    }
}

/// Release - zamašličkovaný snapshot pro produkci
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Release {
    pub id: Uuid,
    pub bundle_version_id: Uuid,
    pub release_id: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

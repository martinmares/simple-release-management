use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Tenant - základní organizační jednotka
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
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

/// Registry authentication type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    None,
    Basic,   // username + password (Docker Hub, generic registries)
    Token,   // username + token (Harbor robot accounts, Quay robot accounts)
    Bearer,  // pure token (GCR, ECR service accounts)
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthType::None => write!(f, "none"),
            AuthType::Basic => write!(f, "basic"),
            AuthType::Token => write!(f, "token"),
            AuthType::Bearer => write!(f, "bearer"),
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
    pub default_project_path: Option<String>,
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub token_encrypted: Option<String>,
    pub role: String,
    pub description: Option<String>,
    pub is_active: bool,
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
    pub auto_tag_enabled: bool,
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
    pub is_archived: bool,
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

    // Target
    pub target_image: String,
    pub app_name: String,
    pub container_name: Option<String>,

    pub created_at: DateTime<Utc>,
}

/// Copy job - konkrétní spuštění copy operace
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyJob {
    pub id: Uuid,
    pub bundle_version_id: Uuid,
    pub target_tag: String,
    pub status: String,
    pub source_registry_id: Option<Uuid>,
    pub target_registry_id: Option<Uuid>,
    pub source_ref_mode: String,
    pub is_release_job: bool,
    pub is_selective: bool,
    pub base_copy_job_id: Option<Uuid>,
    pub release_id: Option<String>,
    pub release_notes: Option<String>,
    pub extra_tags: Option<Vec<String>>,
    pub validate_only: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Copy job image - snapshot + runtime výsledky
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyJobImage {
    pub id: Uuid,
    pub copy_job_id: Uuid,
    pub image_mapping_id: Uuid,
    pub source_image: String,
    pub source_tag: String,
    pub source_registry_id: Option<Uuid>,
    pub target_image: String,
    pub target_tag: String,
    pub source_sha256: Option<String>,
    pub target_sha256: Option<String>,
    pub copy_status: String,
    pub error_message: Option<String>,
    pub copied_at: Option<DateTime<Utc>>,
    pub bytes_copied: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Persisted copy job log line
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyJobLog {
    pub id: Uuid,
    pub copy_job_id: Uuid,
    pub line: String,
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
    pub copy_job_id: Uuid,
    pub release_id: String,
    pub status: String,
    pub source_ref_mode: String,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub is_auto: bool,
    pub auto_reason: Option<String>,
    pub extra_tags: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
}

/// Deploy target - definice build pipeline pro release
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployTarget {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub env_name: String,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub encjson_key_dir: Option<String>,
    #[serde(skip_serializing)]
    pub encjson_private_key_encrypted: Option<String>,
    pub allow_auto_release: bool,
    pub append_env_suffix: bool,
    pub release_manifest_mode: Option<String>,
    pub is_active: bool,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Environment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub color: Option<String>,
    pub source_registry_id: Option<Uuid>,
    pub target_registry_id: Option<Uuid>,
    pub source_project_path: Option<String>,
    pub target_project_path: Option<String>,
    pub source_auth_type: Option<String>,
    pub source_username: Option<String>,
    #[serde(skip_serializing)]
    pub source_password_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub source_token_encrypted: Option<String>,
    pub target_auth_type: Option<String>,
    pub target_username: Option<String>,
    #[serde(skip_serializing)]
    pub target_password_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub target_token_encrypted: Option<String>,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub allow_auto_release: bool,
    pub append_env_suffix: bool,
    pub release_manifest_mode: Option<String>,
    pub encjson_key_dir: Option<String>,
    pub release_env_var_mappings: serde_json::Value,
    pub extra_env_vars: serde_json::Value,
    pub argocd_poll_interval_seconds: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArgocdInstance {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub base_url: String,
    pub auth_type: String,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub token_encrypted: Option<String>,
    pub verify_tls: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KubernetesInstance {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub base_url: String,
    pub oauth_base_url: Option<String>,
    pub auth_type: String,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub token_encrypted: Option<String>,
    pub verify_tls: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnvironmentArgocdApp {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub argocd_instance_id: Uuid,
    pub project_name: String,
    pub application_name: String,
    pub is_active: bool,
    pub ignore_resources: Option<serde_json::Value>,
    pub last_sync_status: Option<String>,
    pub last_health_status: Option<String>,
    pub last_operation_phase: Option<String>,
    pub last_operation_message: Option<String>,
    pub last_revision: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnvironmentKubernetesNamespace {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub kubernetes_instance_id: Uuid,
    pub namespace: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployTargetEnv {
    pub id: Uuid,
    pub deploy_target_id: Uuid,
    pub environment_id: Uuid,
    pub env_repo_id: Option<Uuid>,
    pub env_repo_path: Option<String>,
    pub env_repo_branch: Option<String>,
    pub deploy_repo_id: Option<Uuid>,
    pub deploy_repo_path: Option<String>,
    pub deploy_repo_branch: Option<String>,
    pub allow_auto_release: bool,
    pub append_env_suffix: bool,
    pub is_active: bool,
    pub release_manifest_mode: String,
    pub encjson_key_dir: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnvironmentRegistryPath {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub registry_id: Uuid,
    pub project_path_override: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnvironmentRegistryCredential {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub registry_id: Uuid,
    pub auth_type: String,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub token_encrypted: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnvironmentRegistryAccess {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub registry_id: Uuid,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployTargetEnvVar {
    pub id: Uuid,
    pub deploy_target_id: Uuid,
    pub source_key: String,
    pub target_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployTargetExtraEnvVar {
    pub id: Uuid,
    pub deploy_target_id: Uuid,
    pub key: String,
    pub value: String,
}

/// Git repository configuration per tenant
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GitRepository {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub repo_url: String,
    pub default_branch: String,
    pub git_auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_username: Option<String>,
    #[serde(skip_serializing)]
    pub git_token_encrypted: Option<String>,
    #[serde(skip_serializing)]
    pub git_ssh_key_encrypted: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Encjson key pair per deploy target
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployTargetEncjsonKey {
    pub id: Uuid,
    pub deploy_target_id: Uuid,
    pub public_key: String,
    #[serde(skip_serializing)]
    pub private_key_encrypted: String,
    pub created_at: DateTime<Utc>,
}

/// Deploy job - běh build pipeline
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployJob {
    pub id: Uuid,
    pub release_id: Uuid,
    pub environment_id: Uuid,
    pub deploy_target_id: Option<Uuid>,
    pub deploy_target_env_id: Option<Uuid>,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub commit_sha: Option<String>,
    pub tag_name: Option<String>,
    pub dry_run: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct DeployJobLog {
    pub id: Uuid,
    pub deploy_job_id: Uuid,
    pub log_line: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeployJobDiff {
    pub id: Uuid,
    pub deploy_job_id: Uuid,
    pub files_changed: String,
    pub diff_patch: String,
    pub created_at: DateTime<Utc>,
}

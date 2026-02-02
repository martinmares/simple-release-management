use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Reprezentuje projekt v registry (např. Harbor project)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub description: Option<String>,
}

/// Reprezentuje repository (image) v registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub project: Option<String>,
    pub tags_count: Option<u32>,
}

/// Reprezentuje tag v repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub digest: String,
    pub size: Option<i64>,
    pub created: Option<String>,
}

/// Image manifest s SHA256 digestem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManifest {
    pub digest: String,
    pub media_type: String,
    pub size: i64,
}

/// Credentials pro přístup k registry
#[derive(Debug, Clone)]
pub struct RegistryCredentials {
    pub credentials_path: String,
}

/// Core trait pro všechny registry clients
#[async_trait]
pub trait RegistryClient: Send + Sync {
    /// Vrátí seznam projektů (pokud registry podporuje koncept projektů)
    async fn list_projects(&self) -> Result<Vec<Project>>;

    /// Vrátí seznam repositories
    /// - Pro Harbor: repositories v projektu
    /// - Pro Docker v2: všechny repositories (project ignored)
    async fn list_repositories(&self, project: Option<&str>) -> Result<Vec<Repository>>;

    /// Vrátí seznam tagů pro daný repository
    async fn list_tags(&self, repository: &str) -> Result<Vec<Tag>>;

    /// Vrátí manifest pro image:tag
    async fn get_manifest(&self, image: &str, tag: &str) -> Result<ImageManifest>;

    /// Vrátí SHA256 digest pro image:tag
    async fn get_image_sha(&self, image: &str, tag: &str) -> Result<String>;

    /// Autentizace (většinou se načítá z credentials file)
    async fn authenticate(&self) -> Result<()>;

    /// Zda registry podporuje koncept projektů (Harbor ano, Docker v2 ne)
    fn supports_projects(&self) -> bool;

    /// Zda registry podporuje pokročilé vyhledávání
    fn supports_search(&self) -> bool;
}

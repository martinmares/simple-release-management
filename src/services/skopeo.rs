use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Skopeo credentials pro autentizaci
#[derive(Debug, Clone)]
pub struct SkopeoCredentials {
    pub source_username: Option<String>,
    pub source_password: Option<String>,
    pub target_username: Option<String>,
    pub target_password: Option<String>,
}

/// Skopeo service pro práci s container images
#[derive(Clone)]
pub struct SkopeoService {
    pub skopeo_path: String,
}

/// Informace o image z skopeo inspect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub digest: String,
    pub name: String,
    pub tag: String,
}

/// Progress info pro copy operaci
#[derive(Debug, Clone, Serialize)]
pub struct CopyProgress {
    pub status: CopyStatus,
    pub message: String,
    pub bytes_copied: Option<u64>,
    pub total_bytes: Option<u64>,
}

/// Status copy operace
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CopyStatus {
    Starting,
    InProgress,
    Success,
    Failed,
}

impl SkopeoService {
    pub fn new(skopeo_path: String) -> Self {
        Self { skopeo_path }
    }

    /// Zkontroluje že skopeo je dostupné
    pub async fn check_available(&self) -> Result<bool> {
        let output = Command::new(&self.skopeo_path)
            .arg("--version")
            .output()
            .await
            .context("Failed to execute skopeo")?;

        Ok(output.status.success())
    }

    /// Získá informace o image včetně SHA256 digestu
    pub async fn inspect_image(
        &self,
        image_url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<ImageInfo> {
        info!("Inspecting image: {}", image_url);

        let mut cmd = Command::new(&self.skopeo_path);
        cmd.arg("inspect");

        // Add credentials if provided
        if let (Some(user), Some(pass)) = (username, password) {
            cmd.arg("--creds").arg(format!("{}:{}", user, pass));
        }

        cmd.arg(format!("docker://{}", image_url));

        let output = cmd
            .output()
            .await
            .context("Failed to execute skopeo inspect")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Skopeo inspect failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let inspect_data: serde_json::Value = serde_json::from_str(&stdout)
            .context("Failed to parse skopeo inspect output")?;

        let digest = inspect_data["Digest"]
            .as_str()
            .context("Missing Digest in inspect output")?
            .to_string();

        let name = inspect_data["Name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // Pokusit se získat tag z Name
        let tag = name.split(':').last().unwrap_or("latest").to_string();

        Ok(ImageInfo { digest, name, tag })
    }

    /// Zkopíruje image ze source do target
    pub async fn copy_image(
        &self,
        source_url: &str,
        target_url: &str,
        creds: &SkopeoCredentials,
    ) -> Result<CopyProgress> {
        info!("Copying image from {} to {}", source_url, target_url);

        let mut cmd = Command::new(&self.skopeo_path);
        cmd.arg("copy");

        // Add source credentials if provided
        if let (Some(user), Some(pass)) = (&creds.source_username, &creds.source_password) {
            cmd.arg("--src-creds").arg(format!("{}:{}", user, pass));
        }

        // Add target credentials if provided
        if let (Some(user), Some(pass)) = (&creds.target_username, &creds.target_password) {
            cmd.arg("--dest-creds").arg(format!("{}:{}", user, pass));
        }

        cmd.arg(format!("docker://{}", source_url))
            .arg(format!("docker://{}", target_url))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn skopeo copy")?;

        // Číst stderr pro progress (skopeo píše progress do stderr)
        let stderr = child.stderr.take().context("Failed to get stderr")?;
        let mut reader = BufReader::new(stderr).lines();

        // Číst output v pozadí
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                debug!("Skopeo: {}", line);
            }
        });

        // Počkat na dokončení
        let status = child.wait().await.context("Failed to wait for skopeo")?;

        if status.success() {
            info!(
                "Successfully copied image from {} to {}",
                source_url, target_url
            );
            Ok(CopyProgress {
                status: CopyStatus::Success,
                message: "Image copied successfully".to_string(),
                bytes_copied: None,
                total_bytes: None,
            })
        } else {
            warn!("Failed to copy image from {} to {}", source_url, target_url);
            Ok(CopyProgress {
                status: CopyStatus::Failed,
                message: format!("Copy failed with status: {}", status),
                bytes_copied: None,
                total_bytes: None,
            })
        }
    }

    /// Zkopíruje image s retry logikou
    pub async fn copy_image_with_retry(
        &self,
        source_url: &str,
        target_url: &str,
        creds: &SkopeoCredentials,
        max_retries: u32,
        retry_delay_secs: u64,
    ) -> Result<CopyProgress> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            match self.copy_image(source_url, target_url, creds).await {
                Ok(progress) if progress.status == CopyStatus::Success => {
                    return Ok(progress);
                }
                Ok(progress) if attempts >= max_retries => {
                    return Ok(progress);
                }
                Ok(_) | Err(_) => {
                    if attempts < max_retries {
                        warn!(
                            "Copy attempt {} failed, retrying in {} seconds...",
                            attempts, retry_delay_secs
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay_secs)).await;
                    } else {
                        return Ok(CopyProgress {
                            status: CopyStatus::Failed,
                            message: format!("Failed after {} attempts", attempts),
                            bytes_copied: None,
                            total_bytes: None,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Vyžaduje funkční skopeo v PATH
    async fn test_skopeo_available() {
        let service = SkopeoService::new("skopeo".to_string());
        let available = service.check_available().await.unwrap();
        assert!(available);
    }
}

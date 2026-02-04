use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
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
        cmd.arg("copy")
            .arg("--debug"); // Enable debug output to see what's happening

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

        let output = cmd.output().await.context("Failed to execute skopeo copy")?;

        if output.status.success() {
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            warn!(
                "Failed to copy image from {} to {}\nStatus: {}\nStderr: {}\nStdout: {}",
                source_url, target_url, output.status, stderr, stdout
            );

            Ok(CopyProgress {
                status: CopyStatus::Failed,
                message: format!("Copy failed: {}", stderr.trim()),
                bytes_copied: None,
                total_bytes: None,
            })
        }
    }

    /// Zkopíruje image ze source do target a streamuje logy
    pub async fn copy_image_streaming(
        &self,
        source_url: &str,
        target_url: &str,
        creds: &SkopeoCredentials,
        dest_no_reuse: bool,
        log_tx: Option<&broadcast::Sender<String>>,
    ) -> Result<CopyProgress> {
        info!("Copying image from {} to {}", source_url, target_url);

        let mut cmd = Command::new(&self.skopeo_path);
        cmd.arg("copy")
            .arg("--debug"); // Enable debug output to see what's happening

        if dest_no_reuse {
            cmd.arg("--dest-no-reuse");
        }

        if let (Some(user), Some(pass)) = (&creds.source_username, &creds.source_password) {
            cmd.arg("--src-creds").arg(format!("{}:{}", user, pass));
        }

        if let (Some(user), Some(pass)) = (&creds.target_username, &creds.target_password) {
            cmd.arg("--dest-creds").arg(format!("{}:{}", user, pass));
        }

        cmd.arg(format!("docker://{}", source_url))
            .arg(format!("docker://{}", target_url))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("Failed to execute skopeo copy")?;
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let mut stdout_lines = BufReader::new(stdout).lines();
        let mut stderr_lines = BufReader::new(stderr).lines();
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut status: Option<std::process::ExitStatus> = None;
        let mut last_err = String::new();

        loop {
            if stdout_done && stderr_done && status.is_some() {
                break;
            }

            tokio::select! {
                line = stdout_lines.next_line(), if !stdout_done => {
                    match line {
                        Ok(Some(line)) => {
                            if let Some(tx) = log_tx { let _ = tx.send(line); }
                        }
                        Ok(None) => stdout_done = true,
                        Err(err) => {
                            if let Some(tx) = log_tx { let _ = tx.send(format!("stdout error: {}", err)); }
                            stdout_done = true;
                        }
                    }
                }
                line = stderr_lines.next_line(), if !stderr_done => {
                    match line {
                        Ok(Some(line)) => {
                            last_err = line.clone();
                            if let Some(tx) = log_tx { let _ = tx.send(line); }
                        }
                        Ok(None) => stderr_done = true,
                        Err(err) => {
                            if let Some(tx) = log_tx { let _ = tx.send(format!("stderr error: {}", err)); }
                            stderr_done = true;
                        }
                    }
                }
                exit = child.wait(), if status.is_none() => {
                    match exit {
                        Ok(s) => status = Some(s),
                        Err(err) => {
                            if let Some(tx) = log_tx { let _ = tx.send(format!("process error: {}", err)); }
                            return Err(err.into());
                        }
                    }
                }
            }
        }

        let status = match status {
            Some(status) => status,
            None => {
                return Err(anyhow::anyhow!("Skopeo copy finished without exit status"));
            }
        };

        if status.success() {
            Ok(CopyProgress {
                status: CopyStatus::Success,
                message: "Image copied successfully".to_string(),
                bytes_copied: None,
                total_bytes: None,
            })
        } else {
            let message = if last_err.is_empty() {
                format!("Copy failed: exit status {}", status)
            } else {
                format!("Copy failed: {}", last_err)
            };

            Ok(CopyProgress {
                status: CopyStatus::Failed,
                message,
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
        log_tx: Option<&broadcast::Sender<String>>,
    ) -> Result<CopyProgress> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            let mut progress = self
                .copy_image_streaming(source_url, target_url, creds, false, log_tx)
                .await?;

            if progress.status == CopyStatus::Failed
                && is_reuse_blob_error(&progress.message)
            {
                if let Some(tx) = log_tx {
                    let _ = tx.send("Detected reuse-blob error, retrying with --dest-no-reuse...".to_string());
                }
                progress = self
                    .copy_image_streaming(source_url, target_url, creds, true, log_tx)
                    .await?;
                if let Some(tx) = log_tx {
                    let _ = tx.send("FALLBACK: --dest-no-reuse applied".to_string());
                }
            }

            if progress.status == CopyStatus::Success {
                return Ok(progress);
            }

            if attempts >= max_retries {
                return Ok(progress);
            }

            if let Some(tx) = log_tx {
                let _ = tx.send(format!(
                    "Copy attempt {} failed, retrying in {} seconds...",
                    attempts, retry_delay_secs
                ));
            }
            warn!(
                "Copy attempt {} failed, retrying in {} seconds...",
                attempts, retry_delay_secs
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay_secs)).await;
        }
    }
}

fn is_reuse_blob_error(message: &str) -> bool {
    let msg = message.to_lowercase();
    msg.contains("reuse blob") || msg.contains("reuse-blob") || msg.contains("trying to reuse blob")
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

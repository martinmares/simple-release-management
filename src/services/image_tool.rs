#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{info, warn};

const PROGRESS_MARKER_PREFIX: &str = "__PROGRESS__";

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
pub struct ImageToolService {
    pub tool: ImageTool,
    pub image_tool_path: String,
    pub src_insecure: bool,
    pub dst_insecure: bool,
    pub extra_inspect_args: Vec<String>,
    pub extra_copy_args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageTool {
    Skopeo,
    OciPatch,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OciPatchProgressEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    stage: Option<String>,
    #[serde(default)]
    r#ref: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    current: Option<u64>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    status: Option<String>,
}

impl ImageToolService {
    pub fn new(
        tool: String,
        image_tool_path: String,
        src_insecure: bool,
        dst_insecure: bool,
        extra_inspect_args: Vec<String>,
        extra_copy_args: Vec<String>,
    ) -> Self {
        Self {
            tool: ImageTool::from_env_value(&tool),
            image_tool_path,
            src_insecure,
            dst_insecure,
            extra_inspect_args,
            extra_copy_args,
        }
    }

    /// Zkontroluje že image tool je dostupný
    pub async fn check_available(&self) -> Result<bool> {
        let output = Command::new(&self.image_tool_path)
            .arg("--version")
            .output()
            .await
            .with_context(|| format!("Failed to execute {}", self.tool.display_name()))?;

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

        let mut cmd = Command::new(&self.image_tool_path);
        cmd.arg("inspect");

        // Add credentials if provided
        if let (Some(user), Some(pass)) = (username, password) {
            cmd.arg("--creds").arg(format!("{}:{}", user, pass));
        }

        self.append_inspect_insecure_args(&mut cmd);
        cmd.args(&self.extra_inspect_args);
        cmd.arg(format!("docker://{}", image_url));

        let output = cmd
            .output()
            .await
            .with_context(|| format!("Failed to execute {} inspect", self.tool.display_name()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("{} inspect failed: {}", self.tool.display_name(), stderr);
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

        let mut cmd = Command::new(&self.image_tool_path);
        cmd.arg("copy");

        if self.tool == ImageTool::Skopeo {
            cmd.arg("--debug"); // Skopeo needs debug output for observability.
        }

        if self.tool == ImageTool::OciPatch {
            cmd.arg("--progress-json");
        }

        // Add source credentials if provided
        if let (Some(user), Some(pass)) = (&creds.source_username, &creds.source_password) {
            cmd.arg("--src-creds").arg(format!("{}:{}", user, pass));
        }

        // Add target credentials if provided
        if let (Some(user), Some(pass)) = (&creds.target_username, &creds.target_password) {
            cmd.arg("--dest-creds").arg(format!("{}:{}", user, pass));
        }

        self.append_copy_insecure_args(&mut cmd);
        cmd.args(&self.extra_copy_args);
        cmd.arg(format!("docker://{}", source_url))
            .arg(format!("docker://{}", target_url))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .with_context(|| format!("Failed to execute {} copy", self.tool.display_name()))?;

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

        let mut cmd = Command::new(&self.image_tool_path);
        cmd.arg("copy");

        if self.tool == ImageTool::Skopeo {
            cmd.arg("--debug"); // Skopeo needs debug output for observability.
        }

        if self.tool == ImageTool::OciPatch {
            cmd.arg("--progress-json");
        }

        if dest_no_reuse {
            cmd.arg("--dest-no-reuse");
        }

        if let (Some(user), Some(pass)) = (&creds.source_username, &creds.source_password) {
            cmd.arg("--src-creds").arg(format!("{}:{}", user, pass));
        }

        if let (Some(user), Some(pass)) = (&creds.target_username, &creds.target_password) {
            cmd.arg("--dest-creds").arg(format!("{}:{}", user, pass));
        }

        self.append_copy_insecure_args(&mut cmd);
        cmd.args(&self.extra_copy_args);
        cmd.arg(format!("docker://{}", source_url))
            .arg(format!("docker://{}", target_url))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to execute {} copy", self.tool.display_name()))?;
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let mut stdout_lines = BufReader::new(stdout).lines();
        let mut stderr_lines = BufReader::new(stderr).lines();
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut status: Option<std::process::ExitStatus> = None;
        let mut last_err = String::new();
        let mut bytes_copied: Option<u64> = None;
        let mut total_bytes: Option<u64> = None;

        loop {
            if stdout_done && stderr_done && status.is_some() {
                break;
            }

            tokio::select! {
                line = stdout_lines.next_line(), if !stdout_done => {
                    match line {
                        Ok(Some(line)) => {
                            if self.tool == ImageTool::OciPatch {
                                match serde_json::from_str::<OciPatchProgressEvent>(&line) {
                                    Ok(event) => {
                                        if let Some(tx) = log_tx {
                                            let _ = tx.send(format!(
                                                "{}{}",
                                                PROGRESS_MARKER_PREFIX,
                                                serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string())
                                            ));
                                        }

                                        match event.event_type.as_str() {
                                            "progress" => {
                                                bytes_copied = event.current;
                                                total_bytes = event.total;
                                            }
                                            "phase" => {
                                                let message = match (event.phase.as_deref(), event.r#ref.as_deref()) {
                                                    (Some(phase), Some(reference)) => format!("phase: {} {}", phase, reference),
                                                    (Some(phase), None) => format!("phase: {}", phase),
                                                    _ => String::new(),
                                                };
                                                if !message.is_empty() {
                                                    if let Some(tx) = log_tx { let _ = tx.send(message); }
                                                }
                                            }
                                            "status" | "error" => {
                                                if let Some(message) = event.message {
                                                    last_err = message.clone();
                                                    if let Some(tx) = log_tx { let _ = tx.send(message); }
                                                }
                                            }
                                            "done" => {
                                                if let Some(message) = event.message {
                                                    if let Some(tx) = log_tx { let _ = tx.send(message); }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    Err(_) => {
                                        if let Some(tx) = log_tx { let _ = tx.send(line); }
                                    }
                                }
                            } else if let Some(tx) = log_tx {
                                let _ = tx.send(line);
                            }
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
                bytes_copied,
                total_bytes,
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
                bytes_copied,
                total_bytes,
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

impl ImageToolService {
    fn append_inspect_insecure_args(&self, cmd: &mut Command) {
        match self.tool {
            ImageTool::Skopeo => {
                if self.src_insecure {
                    cmd.arg("--tls-verify=false");
                }
            }
            ImageTool::OciPatch => {
                if self.src_insecure {
                    cmd.arg("--insecure");
                }
            }
        }
    }

    fn append_copy_insecure_args(&self, cmd: &mut Command) {
        match self.tool {
            ImageTool::Skopeo => {
                if self.src_insecure {
                    cmd.arg("--src-tls-verify=false");
                }
                if self.dst_insecure {
                    cmd.arg("--dest-tls-verify=false");
                }
            }
            ImageTool::OciPatch => {
                if self.src_insecure {
                    cmd.arg("--src-insecure");
                }
                if self.dst_insecure {
                    cmd.arg("--dest-insecure");
                }
            }
        }
    }
}

impl ImageTool {
    fn from_env_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "oci-patch" | "oci_patch" => Self::OciPatch,
            _ => Self::Skopeo,
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::Skopeo => "skopeo",
            Self::OciPatch => "oci-patch",
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
        let service = ImageToolService::new(
            "skopeo".to_string(),
            "skopeo".to_string(),
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let available = service.check_available().await.unwrap();
        assert!(available);
    }
}

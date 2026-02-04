use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::env;

/// CLI arguments
#[derive(Debug, Parser)]
#[command(name = "simple-release-management")]
#[command(about = "Simple Release Management - Docker image registry management tool", long_about = None)]
pub struct CliArgs {
    /// Server host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Server port to bind to
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub base_path: String,
    pub skopeo_path: String,
    pub kube_build_app_path: String,
    pub apply_env_path: String,
    pub encjson_path: String,
    pub kubeconform_path: String,
    pub encryption_secret: String,
    pub max_concurrent_copy_jobs: usize,
    pub copy_timeout_seconds: u64,
    pub copy_max_retries: u32,
    pub copy_retry_delay_seconds: u64,
}

impl Config {
    pub fn from_env_and_cli(cli: CliArgs) -> Result<Self> {
        dotenv::dotenv().ok();

        let config = Config {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,

            // CLI argumenty mají prioritu před ENV
            host: cli.host,
            port: cli.port,

            base_path: env::var("BASE_PATH")
                .unwrap_or_default()
                .trim_end_matches('/')
                .to_string(),

            skopeo_path: env::var("SKOPEO_PATH")
                .unwrap_or_else(|_| "skopeo".to_string()),

            kube_build_app_path: env::var("KUBE_BUILD_APP_PATH")
                .unwrap_or_else(|_| "kube_build_app".to_string()),

            apply_env_path: env::var("APPLY_ENV_PATH")
                .unwrap_or_else(|_| "apply-env".to_string()),

            encjson_path: env::var("ENCJSON_PATH")
                .unwrap_or_else(|_| "encjson".to_string()),

            kubeconform_path: env::var("KUBECONFORM_PATH")
                .unwrap_or_else(|_| "kubeconform".to_string()),

            encryption_secret: env::var("ENCRYPTION_SECRET")
                .context("ENCRYPTION_SECRET must be set")?,

            max_concurrent_copy_jobs: env::var("MAX_CONCURRENT_COPY_JOBS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),

            copy_timeout_seconds: env::var("COPY_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),

            copy_max_retries: env::var("COPY_MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),

            copy_retry_delay_seconds: env::var("COPY_RETRY_DELAY_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
        };

        Ok(config)
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

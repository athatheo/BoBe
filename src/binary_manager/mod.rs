//! Binary manager for Ollama — discovery, download, and extraction.
//!
//! Moves Ollama lifecycle from the Swift frontend to the Rust backend.

mod download;
mod extract;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::watch;
use tracing::info;

use crate::error::AppError;

/// Progress update from binary manager operations.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub current_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<u8>,
}

/// Manages Ollama binary discovery, download, and extraction.
pub struct BinaryManager {
    data_dir: PathBuf,
    http_client: Arc<reqwest::Client>,
}

impl BinaryManager {
    pub fn new(data_dir: &Path, http_client: Arc<reqwest::Client>) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            http_client,
        }
    }

    /// Find Ollama binary — check managed location first, then system paths.
    ///
    /// Returns the path to the binary if found.
    pub fn find_ollama(&self) -> Option<PathBuf> {
        // 1. Check managed install location
        let managed = self.managed_binary_path();
        if managed.exists() {
            info!(path = %managed.display(), "binary_manager.found_managed");
            return Some(managed);
        }

        // 2. Check system paths
        let system_paths = [
            "/opt/homebrew/bin/ollama",
            "/usr/local/bin/ollama",
            "/usr/bin/ollama",
        ];

        for path in &system_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                info!(path = %p.display(), "binary_manager.found_system");
                return Some(p);
            }
        }

        // 3. Check PATH via `which`
        if let Ok(path) = which::which("ollama") {
            info!(path = %path.display(), "binary_manager.found_in_path");
            return Some(path);
        }

        None
    }

    /// Ensure Ollama binary is available — download if not found.
    ///
    /// Returns the path to the binary. Sends progress updates via the watch channel.
    pub async fn ensure_ollama(
        &self,
        progress_tx: &watch::Sender<DownloadProgress>,
    ) -> Result<PathBuf, AppError> {
        // Check if already available
        if let Some(path) = self.find_ollama() {
            progress_tx
                .send(DownloadProgress {
                    current_bytes: 0,
                    total_bytes: None,
                    percent: Some(100),
                })
                .ok();
            return Ok(path);
        }

        // Download from GitHub
        let target_path = self.managed_binary_path();
        let parent = target_path
            .parent()
            .ok_or_else(|| AppError::Config("Invalid binary path".into()))?;
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Config(format!("Failed to create binary directory: {e}")))?;

        let archive_path = self.data_dir.join("ollama").join("ollama-darwin.tgz");

        progress_tx
            .send(DownloadProgress {
                current_bytes: 0,
                total_bytes: None,
                percent: Some(0),
            })
            .ok();

        // Download the archive
        download::download_ollama(&self.http_client, &archive_path, |current, total| {
            let percent = total.map(|t| {
                if t > 0 {
                    ((current as f64 / t as f64) * 90.0) as u8
                } else {
                    0
                }
            });
            progress_tx
                .send(DownloadProgress {
                    current_bytes: current,
                    total_bytes: total,
                    percent,
                })
                .ok();
        })
        .await?;

        progress_tx
            .send(DownloadProgress {
                current_bytes: 0,
                total_bytes: None,
                percent: Some(92),
            })
            .ok();

        // Extract the archive
        extract::extract_ollama_archive(&archive_path, &target_path)?;

        // Clean up archive
        let _ = std::fs::remove_file(&archive_path);

        progress_tx
            .send(DownloadProgress {
                current_bytes: 0,
                total_bytes: None,
                percent: Some(100),
            })
            .ok();

        info!(path = %target_path.display(), "binary_manager.installed");
        Ok(target_path)
    }

    fn managed_binary_path(&self) -> PathBuf {
        self.data_dir.join("ollama").join("bin").join("ollama")
    }
}

//! SoulService — manages soul/personality documents for AI prompts.
//!
//! Loads soul content from:
//! 1. Database (via SoulRepository) — preferred, enables UI management
//! 2. Custom file path — for user overrides
//! 3. Default soul shipped with the application — fallback

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::AppError;
use crate::db::SoulRepository;

const DEFAULT_SOUL: &str = "You are BoBe, a helpful AI assistant.";

#[allow(dead_code)]
pub struct SoulService {
    soul_file: Option<PathBuf>,
    soul_repo: Option<Arc<dyn SoulRepository>>,
    cached_content: RwLock<Option<String>>,
}

impl SoulService {
    pub fn new(soul_file: Option<PathBuf>, soul_repo: Option<Arc<dyn SoulRepository>>) -> Self {
        Self {
            soul_file,
            soul_repo,
            cached_content: RwLock::new(None),
        }
    }

    /// Get soul content synchronously (file-based fallback).
    #[allow(dead_code)]
    pub fn get_soul(&self) -> String {
        self.load_soul_from_file()
    }

    /// Get soul content, preferring database over file.
    pub async fn get_soul_async(&self) -> Result<String, AppError> {
        // Try database first
        if let Some(ref repo) = self.soul_repo
            && let Some(content) = self.load_soul_from_db(repo).await
        {
            return Ok(content);
        }
        // Fall back to file-based loading
        Ok(self.load_soul_from_file())
    }

    /// Force reload the soul from file.
    #[allow(dead_code)]
    pub fn reload(&self) -> String {
        let content = self.load_soul_from_file();
        // Clear cache so next async call re-fetches from DB
        if let Ok(mut cached) = self.cached_content.try_write() {
            *cached = None;
        }
        content
    }

    /// Force reload the soul, preferring database.
    #[allow(dead_code)]
    pub async fn reload_async(&self) -> Result<String, AppError> {
        {
            let mut cached = self.cached_content.write().await;
            *cached = None;
        }
        self.get_soul_async().await
    }

    async fn load_soul_from_db(&self, repo: &Arc<dyn SoulRepository>) -> Option<String> {
        match repo.find_enabled().await {
            Ok(souls) if !souls.is_empty() => {
                let names: Vec<&str> = souls.iter().map(|s| s.name.as_str()).collect();
                info!(
                    document_count = souls.len(),
                    ?names,
                    "soul_service.loaded_from_db"
                );
                let combined = souls
                    .into_iter()
                    .map(|s| s.content)
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Some(combined)
            }
            Ok(_) => {
                tracing::debug!("soul_service.no_enabled_souls_in_db");
                None
            }
            Err(e) => {
                warn!(error = %e, "soul_service.db_load_failed");
                None
            }
        }
    }

    fn load_soul_from_file(&self) -> String {
        if let Some(ref path) = self.soul_file {
            let expanded = if path.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    home.join(path.strip_prefix("~").unwrap_or(path))
                } else {
                    path.clone()
                }
            } else {
                path.clone()
            };

            if expanded.exists() {
                match std::fs::read_to_string(&expanded) {
                    Ok(content) => {
                        info!(path = %expanded.display(), "soul_service.loaded_custom");
                        return content;
                    }
                    Err(e) => {
                        warn!(
                            path = %expanded.display(),
                            error = %e,
                            "soul_service.custom_load_failed"
                        );
                    }
                }
            } else {
                warn!(path = %expanded.display(), "soul_service.custom_not_found");
            }
        }

        self.load_default_soul()
    }

    fn load_default_soul(&self) -> String {
        // Try loading from assets directory
        let asset_paths = [
            PathBuf::from("assets/defaults/SOUL.md"),
            dirs::home_dir()
                .map(|h| h.join(".bobe/SOUL.md"))
                .unwrap_or_default(),
        ];

        for path in &asset_paths {
            if path.exists()
                && let Ok(content) = std::fs::read_to_string(path)
            {
                info!(path = %path.display(), "soul_service.loaded_default");
                return content;
            }
        }

        info!("soul_service.using_builtin_default");
        DEFAULT_SOUL.to_owned()
    }
}

impl std::fmt::Debug for SoulService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoulService")
            .field("soul_file", &self.soul_file)
            .field("has_repo", &self.soul_repo.is_some())
            .finish()
    }
}

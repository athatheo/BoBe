//! SoulService — manages soul/personality documents for AI prompts.
//!
//! Loads soul content from:
//! 1. Database (via SoulRepository) — preferred, enables UI management
//! 2. Custom file path — for user overrides
//! 3. Default soul shipped with the application — fallback

use std::path::PathBuf;
use std::sync::Arc;

use tracing::{info, warn};

use crate::constants::DEFAULT_SOUL_FALLBACK;
use crate::db::SoulRepository;
use crate::error::AppError;

pub(crate) struct SoulService {
    soul_file: Option<PathBuf>,
    soul_repo: Option<Arc<dyn SoulRepository>>,
}

impl SoulService {
    pub(crate) fn new(
        soul_file: Option<PathBuf>,
        soul_repo: Option<Arc<dyn SoulRepository>>,
    ) -> Self {
        Self {
            soul_file,
            soul_repo,
        }
    }

    pub(crate) async fn get_soul_async(&self) -> Result<String, AppError> {
        if let Some(ref repo) = self.soul_repo
            && let Some(content) = self.load_soul_from_db(repo).await
        {
            return Ok(content);
        }
        Ok(self.load_soul_from_file().await)
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

    async fn load_soul_from_file(&self) -> String {
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
                match tokio::fs::read_to_string(&expanded).await {
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

        self.load_default_soul().await
    }

    async fn load_default_soul(&self) -> String {
        let asset_paths = [
            PathBuf::from("assets/defaults/SOUL.md"),
            dirs::home_dir()
                .map(|h| h.join(".bobe/SOUL.md"))
                .unwrap_or_default(),
        ];

        for path in &asset_paths {
            if path.exists()
                && let Ok(content) = tokio::fs::read_to_string(path).await
            {
                info!(path = %path.display(), "soul_service.loaded_default");
                return content;
            }
        }

        info!("soul_service.using_builtin_default");
        DEFAULT_SOUL_FALLBACK.to_owned()
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

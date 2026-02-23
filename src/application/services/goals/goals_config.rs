//! Configuration for the goals system.

use std::path::PathBuf;

/// Configuration for the goals system.
///
/// Database is the source of truth for all goals.
/// GOALS.md can optionally seed initial goals on first startup.
#[derive(Debug, Clone)]
pub struct GoalConfig {
    /// Path to GOALS.md file. Defaults to ~/.bobe/GOALS.md if None.
    pub file_path: Option<PathBuf>,
    /// Maximum number of active goals to track.
    pub max_active: u32,
    /// Whether to sync GOALS.md to database on startup.
    pub sync_on_startup: bool,
    /// How often to sync file to database (future use).
    pub sync_interval_minutes: u64,
}

impl Default for GoalConfig {
    fn default() -> Self {
        Self {
            file_path: None,
            max_active: 10,
            sync_on_startup: true,
            sync_interval_minutes: 60,
        }
    }
}

impl GoalConfig {
    /// Get the resolved file path, using default if not specified.
    pub fn resolved_file_path(&self) -> PathBuf {
        if let Some(ref path) = self.file_path {
            path.clone()
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".bobe")
                .join("GOALS.md")
        }
    }
}

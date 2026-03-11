use std::path::PathBuf;

/// Expand a leading `~/` to the user's home directory.
pub(crate) fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// Resolve the BoBe data directory (`$BOBE_DATA_DIR` or `~/.bobe`).
pub(crate) fn bobe_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BOBE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".bobe")
}

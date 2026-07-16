use std::path::PathBuf;

pub(crate) fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// `$BOBE_DATA_DIR` overrides `~/.bobe`.
pub(crate) fn bobe_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BOBE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    std::env::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(crate::constants::BOBE_DATA_DIR_NAME)
}

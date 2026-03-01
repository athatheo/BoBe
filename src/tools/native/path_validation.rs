//! Shared path validation for file system tools.
//!
//! Validates that a path resolves to within the user's home directory or /tmp,
//! with double-canonicalize to mitigate symlink-swap TOCTOU attacks.

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Validate and canonicalize a path, ensuring it is under the home directory or /tmp.
pub(crate) fn validate_path(path: &Path) -> Result<PathBuf, AppError> {
    let canonical = path
        .canonicalize()
        .map_err(|e| AppError::Tool(format!("Cannot resolve path: {e}")))?;

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let tmp = PathBuf::from("/tmp");

    if !canonical.starts_with(&home) && !canonical.starts_with(&tmp) {
        return Err(AppError::Tool(format!(
            "Access denied: path must be under home directory or /tmp. Got: {}",
            canonical.display()
        )));
    }

    // Re-validate after canonicalize to prevent symlink-swap TOCTOU attacks
    let re_canon = canonical
        .canonicalize()
        .map_err(|e| AppError::Tool(format!("Path changed during validation: {e}")))?;
    if !re_canon.starts_with(&home) && !re_canon.starts_with(&tmp) {
        return Err(AppError::Tool(
            "Access denied: path escaped allowed directories after re-validation".into(),
        ));
    }

    Ok(re_canon)
}

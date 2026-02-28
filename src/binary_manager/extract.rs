//! Archive extraction for Ollama binary (tar.gz).
//!
//! Includes path traversal protection.

use std::path::Path;

use tracing::info;

use crate::error::AppError;

/// Extract the `ollama` binary from a `.tgz` archive.
///
/// The archive is expected to contain `bin/ollama` (or just `ollama` at the root).
/// Validates paths to prevent path traversal attacks.
pub fn extract_ollama_archive(archive_path: &Path, output_path: &Path) -> Result<(), AppError> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| AppError::Config(format!("Failed to open archive: {e}")))?;

    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Config(format!("Failed to create output directory: {e}")))?;
    }

    let mut found = false;
    let entries = archive
        .entries()
        .map_err(|e| AppError::Config(format!("Failed to read archive entries: {e}")))?;

    for entry in entries {
        let mut entry =
            entry.map_err(|e| AppError::Config(format!("Failed to read archive entry: {e}")))?;

        let entry_path = entry
            .path()
            .map_err(|e| AppError::Config(format!("Failed to read entry path: {e}")))?
            .into_owned();

        // Path traversal protection: reject paths with .. components
        if entry_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            tracing::warn!(path = %entry_path.display(), "binary_extract.path_traversal_blocked");
            continue;
        }

        // Look for the ollama binary (could be at root or in bin/)
        let file_name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if file_name == "ollama" && !entry.header().entry_type().is_dir() {
            info!(
                entry = %entry_path.display(),
                target = %output_path.display(),
                "binary_extract.extracting_binary"
            );

            let mut output_file = std::fs::File::create(output_path)
                .map_err(|e| AppError::Config(format!("Failed to create output binary: {e}")))?;

            std::io::copy(&mut entry, &mut output_file)
                .map_err(|e| AppError::Config(format!("Failed to extract binary: {e}")))?;

            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(output_path, perms)
                    .map_err(|e| AppError::Config(format!("Failed to set permissions: {e}")))?;
            }

            found = true;
            break;
        }
    }

    if !found {
        return Err(AppError::Config(
            "Ollama binary not found in archive".into(),
        ));
    }

    info!(
        path = %output_path.display(),
        "binary_extract.complete"
    );

    Ok(())
}

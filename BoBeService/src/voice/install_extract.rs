//! Tarball extraction helpers for the voice install service. Streaming
//! Zipformer ships its onnx files with epoch-NN-avg-N suffixes; the
//! daemon's loader inspects bare `encoder.onnx` / `decoder.onnx` /
//! `joiner.onnx`, so we rename on the way in.

use std::path::{Path, PathBuf};

use crate::error::AppError;

pub(super) fn tempfile_dir() -> Result<PathBuf, std::io::Error> {
    let base = std::env::temp_dir();
    let unique = format!(
        "bobe-voice-install-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let path = base.join(unique);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Extract a .tar.bz2 archive whose contents are a single directory.
/// Find that directory inside `tmp_dir`, move it to `final_target`.
/// Also handles encoder/decoder/joiner rename for streaming Zipformer.
pub(super) async fn extract_and_install(
    archive: &Path,
    tmp_dir: &Path,
    final_target: &Path,
) -> Result<(), AppError> {
    let archive_owned = archive.to_path_buf();
    let tmp_owned = tmp_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let file = std::fs::File::open(&archive_owned)
            .map_err(|e| AppError::Internal(format!("open archive: {e}")))?;
        let bz = bzip2::read::BzDecoder::new(file);
        let mut tar = tar::Archive::new(bz);
        tar.unpack(&tmp_owned)
            .map_err(|e| AppError::Internal(format!("untar: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(format!("untar join: {e}")))??;

    let mut extracted: Option<PathBuf> = None;
    let mut entries = tokio::fs::read_dir(tmp_dir)
        .await
        .map_err(|e| AppError::Internal(format!("readdir tmp: {e}")))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("readdir entry: {e}")))?
    {
        let ft = entry
            .file_type()
            .await
            .map_err(|e| AppError::Internal(format!("file_type: {e}")))?;
        if ft.is_dir() {
            extracted = Some(entry.path());
            break;
        }
    }
    let extracted =
        extracted.ok_or_else(|| AppError::Internal("tarball had no directory inside".into()))?;

    // Streaming Zipformer rename: drop epoch suffixes so the daemon's
    // loader finds canonical encoder.onnx / decoder.onnx / joiner.onnx.
    for prefix in ["encoder", "decoder", "joiner"] {
        rename_first_glob(&extracted, prefix, "onnx").await?;
    }

    if let Some(parent) = final_target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(format!("mkdir parent: {e}")))?;
    }
    tokio::fs::rename(&extracted, final_target)
        .await
        .map_err(|e| AppError::Internal(format!("move into models_root: {e}")))?;
    drop(tokio::fs::remove_file(archive).await);
    Ok(())
}

/// Find the first file under `dir` whose name matches `{prefix}*.{ext}`
/// and rename it to `{prefix}.{ext}`. No-op when nothing matches (so
/// non-Zipformer tarballs are unaffected).
async fn rename_first_glob(dir: &Path, prefix: &str, ext: &str) -> Result<(), AppError> {
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| AppError::Internal(format!("readdir for rename: {e}")))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("readdir entry rename: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == format!("{prefix}.{ext}") {
            return Ok(());
        }
        if name.starts_with(prefix) && name.ends_with(&format!(".{ext}")) {
            let from = entry.path();
            let to = dir.join(format!("{prefix}.{ext}"));
            tokio::fs::rename(&from, &to)
                .await
                .map_err(|e| AppError::Internal(format!("rename {prefix}: {e}")))?;
            return Ok(());
        }
    }
    Ok(())
}

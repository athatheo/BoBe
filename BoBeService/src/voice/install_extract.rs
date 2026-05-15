//! Tarball extraction helpers for the voice install service. The only
//! tarball today is Kokoro TTS; daemon-side ASR/VAD/smart-turn were ripped
//! out in M6.B (Mode-B-only pivot) along with their epoch-suffix renaming.

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

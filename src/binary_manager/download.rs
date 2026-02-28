//! Streaming HTTP download for Ollama binary.

use std::path::Path;

use futures::StreamExt;
use tracing::info;

use crate::error::AppError;

const OLLAMA_DARWIN_URL: &str =
    "https://github.com/ollama/ollama/releases/latest/download/ollama-darwin.tgz";

/// Download the Ollama binary archive from GitHub releases.
///
/// Streams the download and calls `on_progress(current_bytes, total_bytes)` periodically.
pub async fn download_ollama(
    client: &reqwest::Client,
    output_path: &Path,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<(), AppError> {
    info!(url = OLLAMA_DARWIN_URL, "binary_download.starting");

    let response = client
        .get(OLLAMA_DARWIN_URL)
        .send()
        .await
        .map_err(|e| AppError::Config(format!("Failed to download Ollama: {e}")))?;

    if !response.status().is_success() {
        return Err(AppError::Config(format!(
            "Ollama download failed with status {}",
            response.status()
        )));
    }

    let total_size = response.content_length();
    info!(total_bytes = ?total_size, "binary_download.content_length");

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Config(format!("Failed to create directory: {e}")))?;
    }

    let mut file = tokio::fs::File::create(output_path)
        .await
        .map_err(|e| AppError::Config(format!("Failed to create output file: {e}")))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_progress: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Config(format!("Download stream error: {e}")))?;

        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| AppError::Config(format!("Failed to write chunk: {e}")))?;

        downloaded += chunk.len() as u64;

        // Report progress every ~1MB
        if downloaded - last_progress > 1_000_000 {
            on_progress(downloaded, total_size);
            last_progress = downloaded;
        }
    }

    // Final progress report
    on_progress(downloaded, total_size);

    info!(
        bytes = downloaded,
        path = %output_path.display(),
        "binary_download.complete"
    );

    Ok(())
}

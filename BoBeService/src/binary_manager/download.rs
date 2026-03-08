//! Streaming HTTP download for Ollama binary.

use std::path::Path;
use std::time::Duration;

use futures::StreamExt;
use tracing::info;

use crate::error::AppError;

const OLLAMA_DARWIN_URL: &str =
    "https://github.com/ollama/ollama/releases/latest/download/ollama-darwin.tgz";
const OLLAMA_DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(30);

pub(crate) async fn download_ollama(
    client: &reqwest::Client,
    output_path: &Path,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<(), AppError> {
    info!(url = OLLAMA_DARWIN_URL, "binary_download.starting");

    let response = client
        .get(OLLAMA_DARWIN_URL)
        .timeout(OLLAMA_DOWNLOAD_TIMEOUT)
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

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Config(format!("Failed to create directory: {e}")))?;
    }

    let partial_path = output_path.with_file_name(format!(
        "{}.part",
        output_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| AppError::Config("Invalid Ollama archive name".into()))?
    ));
    let _ignored = tokio::fs::remove_file(&partial_path).await;

    let mut file = tokio::fs::File::create(&partial_path)
        .await
        .map_err(|e| AppError::Config(format!("Failed to create output file: {e}")))?;

    let result = async {
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_progress: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|e| AppError::Config(format!("Download stream error: {e}")))?;

            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .map_err(|e| AppError::Config(format!("Failed to write chunk: {e}")))?;

            downloaded += chunk.len() as u64;

            if downloaded - last_progress > 1_000_000 {
                on_progress(downloaded, total_size);
                last_progress = downloaded;
            }
        }

        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .map_err(|e| AppError::Config(format!("Failed to flush download file: {e}")))?;
        on_progress(downloaded, total_size);
        Ok::<u64, AppError>(downloaded)
    }
    .await;

    let downloaded = match result {
        Ok(downloaded) => downloaded,
        Err(error) => {
            let _ignored = tokio::fs::remove_file(&partial_path).await;
            return Err(error);
        }
    };

    if output_path.exists() {
        std::fs::remove_file(output_path)
            .map_err(|e| AppError::Config(format!("Failed to replace existing archive: {e}")))?;
    }
    tokio::fs::rename(&partial_path, output_path)
        .await
        .map_err(|e| AppError::Config(format!("Failed to finalize download: {e}")))?;

    info!(
        bytes = downloaded,
        path = %output_path.display(),
        "binary_download.complete"
    );

    Ok(())
}

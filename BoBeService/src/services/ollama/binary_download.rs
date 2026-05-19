//! Atomic `.part` rename; interrupted downloads are discarded on next call.

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
        .await?;

    if !response.status().is_success() {
        return Err(AppError::Config(format!(
            "Ollama download failed with status {}",
            response.status()
        )));
    }

    let total_size = response.content_length();
    info!(total_bytes = ?total_size, "binary_download.content_length");

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let partial_path = output_path.with_file_name(format!(
        "{}.part",
        output_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| AppError::Config("Invalid Ollama archive name".into()))?
    ));
    let _ignored = tokio::fs::remove_file(&partial_path).await;

    let mut file = tokio::fs::File::create(&partial_path).await?;

    let result = async {
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_progress: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
            downloaded += chunk.len() as u64;

            if downloaded - last_progress > 1_000_000 {
                on_progress(downloaded, total_size);
                last_progress = downloaded;
            }
        }

        tokio::io::AsyncWriteExt::flush(&mut file).await?;
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
        std::fs::remove_file(output_path)?;
    }
    tokio::fs::rename(&partial_path, output_path).await?;

    info!(
        bytes = downloaded,
        path = %output_path.display(),
        "binary_download.complete"
    );

    Ok(())
}

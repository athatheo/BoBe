use std::path::{Path, PathBuf};

use crate::error::AppError;

fn parent(path: &Path) -> Result<&Path, AppError> {
    path.parent().ok_or_else(|| {
        AppError::Internal(format!("durable path has no parent: {}", path.display()))
    })
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data");
    parent(path)
        .unwrap_or_else(|_| Path::new("."))
        .join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()))
}

pub(crate) async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let directory = parent(path)?;
    tokio::fs::create_dir_all(directory).await?;
    let temporary = temporary_path(path);

    let result = async {
        use tokio::io::AsyncWriteExt;

        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, path).await?;
        sync_directory(directory).await
    }
    .await;

    if result.is_err() {
        let _ignored = tokio::fs::remove_file(&temporary).await;
    }
    result
}

pub(crate) fn atomic_write_sync(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write;

    let directory = parent(path)?;
    std::fs::create_dir_all(directory)?;
    let temporary = temporary_path(path);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        sync_directory_sync(directory)
    })();
    if result.is_err() {
        let _ignored = std::fs::remove_file(&temporary);
    }
    result
}

pub(crate) async fn durable_remove_file(path: &Path) -> Result<bool, AppError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {
            sync_directory(parent(path)?).await?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::Io(error)),
    }
}

pub(crate) async fn durable_remove_dir_all(path: &Path) -> Result<bool, AppError> {
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => {
            sync_directory(parent(path)?).await?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::Io(error)),
    }
}

async fn sync_directory(path: &Path) -> Result<(), AppError> {
    let directory = tokio::fs::File::open(path).await?;
    directory.sync_all().await?;
    Ok(())
}

fn sync_directory_sync(path: &Path) -> Result<(), AppError> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_replace_and_delete_round_trip() {
        let dir = std::env::temp_dir().join(format!("bobe-durable-{}", uuid::Uuid::new_v4()));
        let path = dir.join("critical.txt");
        atomic_write(&path, b"first").await.unwrap();
        atomic_write(&path, b"second").await.unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"second");
        assert!(durable_remove_file(&path).await.unwrap());
        assert!(!durable_remove_file(&path).await.unwrap());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}

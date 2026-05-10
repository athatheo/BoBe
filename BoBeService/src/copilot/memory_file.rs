//! Single-writer to `~/.bobe/memory.md`; atomic rename keeps readers from seeing torn writes.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::error::AppError;

const DEFAULT_BODY: &str =
    "# BoBe Memory\n\n## Profile\n\n## Active Goals\n\n## Long-term\n\n## Recent\n";

/// Soft cap enforced only by the nightly consolidation worker.
pub(crate) const TARGET_MAX_BYTES: usize = 50 * 1024;

pub(crate) struct MemoryFile {
    path: PathBuf,
    write_lock: Arc<Mutex<()>>,
}

/// Hold across consolidate cycle so appends queue behind it instead of being clobbered.
pub(crate) struct WriterGuard<'a> {
    file: &'a MemoryFile,
    _guard: OwnedMutexGuard<()>,
}

impl WriterGuard<'_> {
    pub(crate) async fn read(&self) -> Result<String, AppError> {
        self.file.read_unlocked().await
    }
    pub(crate) async fn replace_all(&self, body: String) -> Result<(), AppError> {
        commit(&self.file.path, &body).await
    }
}

impl MemoryFile {
    pub(crate) fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            path,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

    pub(crate) async fn acquire_writer(&self) -> WriterGuard<'_> {
        let guard = Arc::clone(&self.write_lock).lock_owned().await;
        WriterGuard {
            file: self,
            _guard: guard,
        }
    }

    pub(crate) async fn read(&self) -> Result<String, AppError> {
        self.read_unlocked().await
    }

    async fn read_unlocked(&self) -> Result<String, AppError> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DEFAULT_BODY.to_string()),
            Err(e) => Err(AppError::Io(e)),
        }
    }

    /// Creates section if missing; prefixes each entry with ISO-8601 date.
    pub(crate) async fn append_under(&self, section: &str, entry: &str) -> Result<(), AppError> {
        let _guard = self.write_lock.lock().await;
        let body = self.read_unlocked().await?;
        let now: DateTime<Utc> = SystemTime::now().into();
        let date = now.format("%Y-%m-%d %H:%M");
        let line = format!("- {date} — {}", entry.trim());
        let new_body = upsert_bullet(&body, section, &line);
        commit(&self.path, &new_body).await
    }

    /// For consolidation requiring lock across read+write, see [`acquire_writer`].
    #[allow(
        dead_code,
        reason = "Phase 2: consumed by the consolidation worker added in Phase 4"
    )]
    pub(crate) async fn replace_all(&self, body: String) -> Result<(), AppError> {
        let _guard = self.write_lock.lock().await;
        commit(&self.path, &body).await
    }
}

fn upsert_bullet(body: &str, heading: &str, line: &str) -> String {
    let header = format!("## {heading}");
    let Some(start) = body.find(&header) else {
        let mut out = body.trim_end().to_string();
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&header);
        out.push_str("\n\n");
        out.push_str(line);
        out.push('\n');
        return out;
    };

    let body_after_header = &body[start..];
    let next_section = body_after_header[header.len()..]
        .find("\n## ")
        .map_or(body.len(), |i| start + header.len() + i);

    let (head, rest) = body.split_at(next_section);
    let head_trimmed = head.trim_end();
    let mut out = head_trimmed.to_string();
    out.push('\n');
    out.push_str(line);
    out.push('\n');
    if !rest.is_empty() {
        if !rest.starts_with('\n') {
            out.push('\n');
        }
        out.push_str(rest);
    }
    out
}

async fn commit(path: &Path, body: &str) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Internal(format!("memory.md has no parent: {}", path.display()))
    })?;
    tokio::fs::create_dir_all(parent).await?;

    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("memory")
    ));
    tokio::fs::write(&tmp, body).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tempdir() -> PathBuf {
        let suffix: String = Uuid::new_v4()
            .as_simple()
            .to_string()
            .chars()
            .take(8)
            .collect();
        let d = PathBuf::from(format!("/tmp/bbm-{suffix}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn read_returns_default_when_missing() {
        let dir = tempdir();
        let mem = MemoryFile::new(dir.join("memory.md"));
        let body = mem.read().await.unwrap();
        assert!(body.contains("# BoBe Memory"));
        assert!(body.contains("## Recent"));
    }

    #[tokio::test]
    async fn append_under_existing_section() {
        let dir = tempdir();
        let mem = MemoryFile::new(dir.join("memory.md"));
        mem.append_under("Recent", "User asked about Foo")
            .await
            .unwrap();
        mem.append_under("Recent", "User asked about Bar")
            .await
            .unwrap();
        let body = mem.read().await.unwrap();

        assert!(body.contains("User asked about Foo"));
        assert!(body.contains("User asked about Bar"));
        assert!(
            body.matches("## Recent").count() == 1,
            "no duplicate sections"
        );
    }

    #[tokio::test]
    async fn append_creates_missing_section() {
        let dir = tempdir();
        let mem = MemoryFile::new(dir.join("memory.md"));
        mem.append_under("BrandNew", "thing").await.unwrap();
        let body = mem.read().await.unwrap();
        assert!(body.contains("## BrandNew"));
        assert!(body.contains("- ") && body.contains("thing"));
    }

    #[tokio::test]
    async fn replace_all_overwrites() {
        let dir = tempdir();
        let mem = MemoryFile::new(dir.join("memory.md"));
        mem.append_under("Recent", "first").await.unwrap();
        mem.replace_all("# fresh\n".to_string()).await.unwrap();
        assert_eq!(mem.read().await.unwrap(), "# fresh\n");
    }

    #[tokio::test]
    async fn concurrent_appends_are_serialized() {
        let dir = tempdir();
        let mem = MemoryFile::new(dir.join("memory.md"));
        let mut handles = Vec::new();
        for i in 0..10 {
            let m = Arc::clone(&mem);
            handles.push(tokio::spawn(async move {
                m.append_under("Recent", &format!("entry {i}")).await
            }));
        }
        for h in handles {
            h.await.unwrap().unwrap();
        }
        let body = mem.read().await.unwrap();
        for i in 0..10 {
            assert!(body.contains(&format!("entry {i}")), "missing entry {i}");
        }
    }
}

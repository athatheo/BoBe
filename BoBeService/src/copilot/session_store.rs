//! Chat rotates daily; cross-day continuity comes from `memory.md`, not chat history.

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use github_copilot_sdk::SessionId;

use crate::error::AppError;

use super::types::WorkerClass;

pub(crate) struct SessionStore {
    workers_root: PathBuf,
    retired_root: PathBuf,
}

impl SessionStore {
    pub(crate) fn new(data_dir: &std::path::Path) -> Self {
        Self {
            workers_root: data_dir.join("workers"),
            retired_root: data_dir.join("retired-copilot-sessions"),
        }
    }

    /// Only Chat uses the date; other classes share a stable `session.id` file.
    pub(crate) fn id_path(&self, class: WorkerClass, now_local: DateTime<Local>) -> PathBuf {
        let dir = self.workers_root.join(class.name());
        if class == WorkerClass::Chat {
            dir.join(format!("session-{}.id", now_local.format("%Y-%m-%d")))
        } else {
            dir.join("session.id")
        }
    }

    pub(crate) async fn load(
        &self,
        class: WorkerClass,
        now_local: DateTime<Local>,
    ) -> Result<Option<SessionId>, AppError> {
        let path = self.id_path(class, now_local);
        match tokio::fs::read_to_string(&path).await {
            Ok(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(SessionId::new(trimmed)))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AppError::Io(e)),
        }
    }

    pub(crate) async fn save(
        &self,
        class: WorkerClass,
        now_local: DateTime<Local>,
        id: &SessionId,
    ) -> Result<(), AppError> {
        let path = self.id_path(class, now_local);
        crate::util::durable_fs::atomic_write(&path, id.as_str().as_bytes()).await
    }

    pub(crate) async fn load_all(&self) -> Result<Vec<SessionId>, AppError> {
        let mut ids = BTreeSet::new();
        collect_ids(&self.workers_root, &mut ids).await?;
        collect_ids(&self.retired_root, &mut ids).await?;
        Ok(ids.into_iter().map(SessionId::new).collect())
    }

    /// Move every active ID to a ledger that create/resume never reads. Hard
    /// engine reloads can then start fresh while privacy purge keeps retrying
    /// any SDK deletion that failed.
    pub(crate) async fn retire_active(&self) -> Result<Vec<SessionId>, AppError> {
        let mut ids = BTreeSet::new();
        collect_ids(&self.workers_root, &mut ids).await?;
        for id in &ids {
            self.write_retired_id(id).await?;
        }
        crate::util::durable_fs::durable_remove_dir_all(&self.workers_root).await?;
        Ok(ids.into_iter().map(SessionId::new).collect())
    }

    pub(crate) async fn retire(
        &self,
        class: WorkerClass,
        now_local: DateTime<Local>,
        id: &SessionId,
    ) -> Result<(), AppError> {
        self.write_retired_id(id.as_str()).await?;
        crate::util::durable_fs::durable_remove_file(&self.id_path(class, now_local))
            .await
            .map(|_| ())
    }

    async fn write_retired_id(&self, id: &str) -> Result<(), AppError> {
        let path = self
            .retired_root
            .join(format!("session-{}.id", uuid::Uuid::new_v4()));
        crate::util::durable_fs::atomic_write(&path, id.as_bytes()).await
    }

    pub(crate) async fn retire_untracked(&self, id: &SessionId) -> Result<(), AppError> {
        self.write_retired_id(id.as_str()).await
    }

    pub(crate) async fn clear_retired(&self) -> Result<(), AppError> {
        crate::util::durable_fs::durable_remove_dir_all(&self.retired_root)
            .await
            .map(|_| ())
    }

    pub(crate) async fn clear_all(&self) -> Result<(), AppError> {
        crate::util::durable_fs::durable_remove_dir_all(&self.workers_root).await?;
        self.clear_retired().await
    }
}

async fn collect_ids(root: &std::path::Path, ids: &mut BTreeSet<String>) -> Result<(), AppError> {
    let mut class_dirs = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(AppError::Io(error)),
    };

    while let Some(class_dir) = class_dirs.next_entry().await? {
        if class_dir.file_type().await?.is_file() {
            collect_id_file(&class_dir.path(), ids).await?;
            continue;
        }
        if class_dir.file_type().await?.is_dir() {
            let mut files = tokio::fs::read_dir(class_dir.path()).await?;
            while let Some(file) = files.next_entry().await? {
                if file.file_type().await?.is_file() {
                    collect_id_file(&file.path(), ids).await?;
                }
            }
        }
    }
    Ok(())
}

async fn collect_id_file(
    path: &std::path::Path,
    ids: &mut BTreeSet<String>,
) -> Result<(), AppError> {
    if path.extension().and_then(std::ffi::OsStr::to_str) != Some("id") {
        return Ok(());
    }
    let contents = tokio::fs::read_to_string(path).await?;
    let id = contents.trim();
    if !id.is_empty() {
        ids.insert(id.to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn tempdir() -> PathBuf {
        let suffix: String = Uuid::new_v4()
            .as_simple()
            .to_string()
            .chars()
            .take(8)
            .collect();
        let d = PathBuf::from(format!("/tmp/bbss-{suffix}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn load_returns_none_when_missing() {
        let store = SessionStore::new(&tempdir());
        let now = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        assert!(store.load(WorkerClass::Goals, now).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let store = SessionStore::new(&tempdir());
        let now = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        let id = SessionId::new("abc-123");
        store.save(WorkerClass::Goals, now, &id).await.unwrap();
        let loaded = store.load(WorkerClass::Goals, now).await.unwrap();
        assert_eq!(loaded, Some(id));
    }

    #[tokio::test]
    async fn chat_rotates_daily() {
        let store = SessionStore::new(&tempdir());
        let mon = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        let tue = Local.with_ymd_and_hms(2026, 5, 9, 10, 0, 0).unwrap();

        store
            .save(WorkerClass::Chat, mon, &SessionId::new("mon-id"))
            .await
            .unwrap();
        assert!(store.load(WorkerClass::Chat, tue).await.unwrap().is_none());
        assert_eq!(
            store.load(WorkerClass::Chat, mon).await.unwrap(),
            Some(SessionId::new("mon-id"))
        );
    }

    #[tokio::test]
    async fn load_all_includes_historical_chat_and_deduplicates_ids() {
        let root = tempdir();
        let store = SessionStore::new(&root);
        let mon = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        let tue = Local.with_ymd_and_hms(2026, 5, 9, 10, 0, 0).unwrap();

        store
            .save(WorkerClass::Chat, mon, &SessionId::new("chat-mon"))
            .await
            .unwrap();
        store
            .save(WorkerClass::Chat, tue, &SessionId::new("chat-tue"))
            .await
            .unwrap();
        store
            .save(WorkerClass::Goals, tue, &SessionId::new("shared"))
            .await
            .unwrap();
        store
            .save(WorkerClass::Vision, tue, &SessionId::new("shared"))
            .await
            .unwrap();
        tokio::fs::write(root.join("workers/chat/notes.txt"), "ignored")
            .await
            .unwrap();

        let ids = store
            .load_all()
            .await
            .unwrap()
            .into_iter()
            .map(|id| id.as_str().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["chat-mon", "chat-tue", "shared"]);
    }

    #[tokio::test]
    async fn retire_active_removes_resume_paths_but_keeps_purge_ledger() {
        let root = tempdir();
        let store = SessionStore::new(&root);
        let now = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        store
            .save(WorkerClass::Chat, now, &SessionId::new("chat-id"))
            .await
            .unwrap();

        let retired = store.retire_active().await.unwrap();

        assert_eq!(retired, [SessionId::new("chat-id")]);
        assert!(store.load(WorkerClass::Chat, now).await.unwrap().is_none());
        assert_eq!(store.load_all().await.unwrap(), retired);
        store.clear_all().await.unwrap();
        assert!(store.load_all().await.unwrap().is_empty());
    }
}

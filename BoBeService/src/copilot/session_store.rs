//! Chat rotates daily; cross-day continuity comes from `memory.md`, not chat history.

use std::path::PathBuf;

use chrono::{DateTime, Local};
use github_copilot_sdk::SessionId;

use crate::error::AppError;

use super::types::WorkerClass;

pub(crate) struct SessionStore {
    workers_root: PathBuf,
}

impl SessionStore {
    pub(crate) fn new(data_dir: &std::path::Path) -> Self {
        Self {
            workers_root: data_dir.join("workers"),
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

    pub(crate) async fn forget(
        &self,
        class: WorkerClass,
        now_local: DateTime<Local>,
    ) -> Result<(), AppError> {
        let path = self.id_path(class, now_local);
        crate::util::durable_fs::durable_remove_file(&path)
            .await
            .map(|_| ())
    }
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
    async fn forget_is_idempotent() {
        let store = SessionStore::new(&tempdir());
        let now = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        store.forget(WorkerClass::Goals, now).await.unwrap();

        store
            .save(WorkerClass::Goals, now, &SessionId::new("x"))
            .await
            .unwrap();
        store.forget(WorkerClass::Goals, now).await.unwrap();
        assert!(store.load(WorkerClass::Goals, now).await.unwrap().is_none());
    }
}

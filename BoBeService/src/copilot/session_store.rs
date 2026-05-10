//! Chat rotates daily; cross-day continuity comes from `memory.md`, not chat history.

pub(crate) const CHAT_RETENTION_DAYS: i64 = 7;

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
        let parent = path.parent().ok_or_else(|| {
            AppError::Internal(format!("session-id path has no parent: {}", path.display()))
        })?;
        tokio::fs::create_dir_all(parent).await?;

        let tmp = parent.join(format!(
            ".{}.tmp",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("session")
        ));
        tokio::fs::write(&tmp, id.as_str()).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub(crate) async fn forget(
        &self,
        class: WorkerClass,
        now_local: DateTime<Local>,
    ) -> Result<(), AppError> {
        let path = self.id_path(class, now_local);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AppError::Io(e)),
        }
    }

    pub(crate) async fn load_path(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<SessionId>, AppError> {
        match tokio::fs::read_to_string(path).await {
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

    pub(crate) async fn old_chat_sessions(
        &self,
        now_local: DateTime<Local>,
        retention_days: i64,
    ) -> Result<Vec<(std::path::PathBuf, SessionId)>, AppError> {
        let chat_dir = self.workers_root.join(WorkerClass::Chat.name());
        let mut read_dir = match tokio::fs::read_dir(&chat_dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(AppError::Io(e)),
        };

        let cutoff = now_local.date_naive() - chrono::Duration::days(retention_days);
        let mut victims = Vec::new();

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(date_part) = name
                .strip_prefix("session-")
                .and_then(|rest| rest.strip_suffix(".id"))
            else {
                continue;
            };
            let Ok(file_date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") else {
                continue;
            };
            if file_date < cutoff
                && let Ok(Some(id)) = self.load_path(&path).await
            {
                victims.push((path, id));
            }
        }

        Ok(victims)
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

    #[tokio::test]
    async fn old_chat_sessions_returns_only_files_past_cutoff() {
        let store = SessionStore::new(&tempdir());
        let today = Local.with_ymd_and_hms(2026, 5, 15, 10, 0, 0).unwrap();
        let yesterday = Local.with_ymd_and_hms(2026, 5, 14, 10, 0, 0).unwrap();
        let week_ago = Local.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
        let two_weeks_ago = Local.with_ymd_and_hms(2026, 5, 1, 10, 0, 0).unwrap();

        for (when, id) in [
            (today, "today-id"),
            (yesterday, "yest-id"),
            (week_ago, "week-id"),
            (two_weeks_ago, "old-id"),
        ] {
            store
                .save(WorkerClass::Chat, when, &SessionId::new(id))
                .await
                .unwrap();
        }

        let victims = store.old_chat_sessions(today, 7).await.unwrap();

        // week_ago is exactly 7 days; cutoff is `< today - 7d` so it's kept.
        assert_eq!(victims.len(), 1);
        assert_eq!(victims[0].1, SessionId::new("old-id"));
    }
}

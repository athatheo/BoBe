//! Single-writer: worker returns the body; only `MemoryFile::replace_all` writes memory.md.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, NaiveTime, TimeZone, Utc};
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::AppError;

use super::memory_file::{MemoryFile, TARGET_MAX_BYTES};
use super::registry::WorkerRegistry;
use super::types::JobInput;

const DEFAULT_FIRE_AT: (u32, u32) = (3, 0);

pub(crate) struct ConsolidationTrigger {
    workers: Arc<WorkerRegistry>,
    memory_file: Arc<MemoryFile>,
    fire_at: NaiveTime,
}

impl ConsolidationTrigger {
    pub(crate) fn new(workers: Arc<WorkerRegistry>, memory_file: Arc<MemoryFile>) -> Self {
        let fire_at = NaiveTime::from_hms_opt(DEFAULT_FIRE_AT.0, DEFAULT_FIRE_AT.1, 0)
            .unwrap_or(NaiveTime::MIN);
        Self {
            workers,
            memory_file,
            fire_at,
        }
    }

    pub(crate) async fn run(self, mut shutdown: broadcast::Receiver<()>) {
        loop {
            let wait = duration_until_next(self.fire_at, Utc::now());
            tracing::info!(
                wait_secs = wait.as_secs(),
                fire_at = %self.fire_at,
                "consolidation_trigger.sleeping"
            );

            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                _ = shutdown.recv() => {
                    tracing::info!("consolidation_trigger.stopped");
                    return;
                }
            }

            match self.consolidate_once().await {
                Ok(_) => {}
                Err(AppError::Conflict(reason)) => {
                    tracing::info!(reason = %reason, "consolidation_trigger.skipped");
                }
                Err(e) => {
                    tracing::warn!(err = %e, "consolidation_trigger.run_failed");
                }
            }
        }
    }

    /// Optimistic concurrency: reads lock-free, holds writer lock only for verify+commit.
    pub(crate) async fn consolidate_once(&self) -> Result<(), AppError> {
        let started = Instant::now();

        let before = self.memory_file.read().await?;
        let before_bytes = before.len();

        let worker = self.workers.consolidate().await?;

        let job = JobInput {
            job_id: Uuid::new_v4(),
            kind: "consolidate".into(),
            instructions: format!(
                "Read the markdown in `input.body`. Produce a pruned, deduplicated \
                 version that preserves all load-bearing facts but drops near-duplicates, \
                 stale Recent entries that aren't promoted to Long-term, and noise. \
                 Target ≤ {TARGET_MAX_BYTES} bytes. \
                 Return JSON {{\"body\": \"<the new full markdown>\"}} — do not write \
                 to memory.md yourself; the daemon owns that file."
            ),
            input: json!({ "body": before }),
        };

        let out = worker
            .submit(job)
            .await
            .map_err(|e| AppError::Internal(format!("consolidate submit: {e}")))?;

        let new_body = out
            .output
            .get("body")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::Internal("consolidate worker returned no `output.body`".into())
            })?
            .to_string();

        if new_body.trim().is_empty() {
            return Err(AppError::Internal(
                "consolidate worker returned empty body — refusing to clobber memory.md".into(),
            ));
        }

        let after_bytes = new_body.len();

        let writer = self.memory_file.acquire_writer().await;
        let current = writer.read().await?;
        if current != before {
            tracing::warn!(
                before_bytes,
                current_bytes = current.len(),
                "consolidation_trigger.aborted_concurrent_write"
            );
            return Err(AppError::Conflict(
                "memory.md changed during consolidation; skipping this pass".into(),
            ));
        }
        writer.replace_all(new_body).await?;
        drop(writer);

        let took = started.elapsed();
        tracing::info!(
            before_bytes,
            after_bytes,
            saved_bytes = before_bytes.saturating_sub(after_bytes),
            took_ms = took.as_millis(),
            "consolidation_trigger.run_complete"
        );

        Ok(())
    }
}

fn duration_until_next(fire_at: NaiveTime, now_utc: DateTime<Utc>) -> Duration {
    let now_local = now_utc.with_timezone(&Local);
    let today_target = Local
        .from_local_datetime(&now_local.date_naive().and_time(fire_at))
        .single();

    let target = match today_target {
        Some(t) if t > now_local => t,
        _ => {
            let tomorrow = now_local.date_naive().succ_opt().unwrap_or_else(|| {
                tracing::warn!("date overflow; defaulting to 24h");
                now_local.date_naive()
            });
            Local
                .from_local_datetime(&tomorrow.and_time(fire_at))
                .single()
                .unwrap_or(now_local + chrono::Duration::hours(24))
        }
    };

    let delta = target.signed_duration_since(now_local);
    #[allow(clippy::duration_suboptimal_units, reason = "from_days is unstable")]
    let one_day = Duration::from_secs(60 * 60 * 24);
    delta.to_std().unwrap_or(one_day)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn duration_until_next_picks_today_when_in_future() {
        let now_local = Local.with_ymd_and_hms(2026, 5, 8, 1, 0, 0).unwrap();
        let now_utc = now_local.with_timezone(&Utc);
        let fire = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let d = duration_until_next(fire, now_utc);
        assert!(d.as_secs() <= 2 * 60 * 60 + 1);
        assert!(d.as_secs() >= 2 * 60 * 60 - 1);
    }

    #[test]
    fn duration_until_next_picks_tomorrow_when_past() {
        let now_local = Local.with_ymd_and_hms(2026, 5, 8, 4, 0, 0).unwrap();
        let now_utc = now_local.with_timezone(&Utc);
        let fire = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let d = duration_until_next(fire, now_utc);
        assert!(d.as_secs() <= 23 * 60 * 60 + 1);
        assert!(d.as_secs() >= 23 * 60 * 60 - 1);
    }
}

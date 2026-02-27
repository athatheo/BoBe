//! Startup data-integrity checks.
//!
//! Runs once at boot to fix state that can't survive an unclean shutdown:
//! orphaned running jobs and corrupt embedding columns.

use sqlx::sqlite::SqlitePool;
use tracing::{info, warn};

use crate::db::AgentJobRepository;
use crate::models::types::AgentJobStatus;

/// Run all integrity checks. Best-effort — failures are logged, never fatal.
pub async fn run(pool: &SqlitePool, agent_job_repo: &dyn AgentJobRepository) {
    mark_orphaned_jobs(agent_job_repo).await;
    repair_corrupt_embeddings(pool).await;
}

async fn mark_orphaned_jobs(repo: &dyn AgentJobRepository) {
    match repo.find_by_status(AgentJobStatus::Running).await {
        Ok(orphans) if !orphans.is_empty() => {
            info!(count = orphans.len(), "integrity.orphaned_jobs");
            for mut job in orphans {
                job.mark_failed("Orphaned on restart".to_string(), None);
                if let Err(e) = repo.save(&job).await {
                    warn!(job_id = %job.id, error = %e, "integrity.orphan_save_failed");
                }
            }
        }
        Ok(_) => {}
        Err(e) => warn!(error = %e, "integrity.orphan_check_failed"),
    }
}

/// NULL-out embedding columns that aren't valid JSON arrays.
async fn repair_corrupt_embeddings(pool: &SqlitePool) {
    let mut total = 0u64;
    for table in ["memories", "observations"] {
        let sql = format!(
            "UPDATE {table} SET embedding = NULL \
             WHERE embedding IS NOT NULL AND embedding NOT LIKE '[%'"
        );
        match sqlx::query(&sql).execute(pool).await {
            Ok(r) if r.rows_affected() > 0 => {
                let n = r.rows_affected();
                warn!(table, rows = n, "integrity.repaired_embeddings");
                total += n;
            }
            Err(e) => warn!(error = %e, table, "integrity.embedding_repair_failed"),
            _ => {}
        }
    }
    if total > 0 {
        info!(total, "integrity.embeddings_repaired");
    }
}

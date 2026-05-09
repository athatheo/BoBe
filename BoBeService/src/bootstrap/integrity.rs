//! Startup data-integrity checks. Marks orphaned `running` agent jobs
//! as failed so the next scheduler tick doesn't see ghost work.

use tracing::{info, warn};

use crate::db::AgentJobRepository;
use crate::models::types::AgentJobStatus;

pub(crate) async fn run(agent_job_repo: &dyn AgentJobRepository) {
    mark_orphaned_jobs(agent_job_repo).await;
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

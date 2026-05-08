//! SDK-pivot spike: drive one Copilot SDK session through one job
//! end-to-end. Run via `bobe spike-copilot`. Manual sanity check, not a
//! production code path.

#![allow(
    clippy::print_stdout,
    reason = "spike subcommand: stdout is the user-facing report"
)]

use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use super::memory_file::MemoryFile;
use super::registry::WorkerRegistry;
use super::worker::JobInput;

pub(crate) async fn run() -> anyhow::Result<()> {
    let data_dir = crate::util::paths::bobe_data_dir();
    tokio::fs::create_dir_all(&data_dir).await?;

    let memory_file = MemoryFile::new(data_dir.join("memory.md"));
    let registry = WorkerRegistry::new(Arc::clone(&memory_file));

    tracing::info!("starting copilot SDK spike");
    let worker = registry.goals().await?;

    let job = JobInput {
        job_id: Uuid::new_v4(),
        kind: "goal_extraction".into(),
        instructions: "Extract goals from `input.text`. Return JSON shaped \
             {\"output\":{\"goals\":[{\"title\":\"...\",\"why\":\"...\"}]}}. \
             Aim for 1-3 goals."
            .into(),
        input: json!({
            "text": "I want to ship the BoBe v1 release this quarter. \
                I keep procrastinating on the build pipeline. \
                Tomorrow I should make tea instead of coffee."
        }),
    };

    tracing::info!(job_id = %job.job_id, "submitting job");
    let out = worker.submit(job).await?;
    tracing::info!(job_id = %out.job_id, "spike round-trip OK");

    println!("\n--- spike result ---");
    println!("job_id: {}", out.job_id);
    println!("output: {}", serde_json::to_string_pretty(&out.output)?);
    if !out.text.is_empty() {
        println!("---");
        println!("raw assistant text:");
        println!("{}", out.text);
    }
    if let Some(err) = out.error {
        println!("error:  {err}");
    }

    registry.shutdown_all().await;
    Ok(())
}

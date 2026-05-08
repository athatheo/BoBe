//! Phase-1 spike: drive one Copilot CLI worker through one job end-to-end.
//! Run via `bobe spike-copilot`. Intended as a manual sanity check, not a
//! production code path — no tests; the worker/hook/mux modules are
//! unit-tested independently.

#![allow(
    clippy::print_stdout,
    reason = "spike subcommand: stdout is the user-facing report"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use super::mux::Mux;
use super::worker::{CopilotWorker, JobInput, WorkerConfig};

pub(crate) async fn run() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "cannot resolve directory of current executable {}",
            exe.display()
        )
    })?;
    let hook_binary = exe_dir.join("bobe-copilot-hook");
    if !hook_binary.exists() {
        anyhow::bail!(
            "hook binary not found at {} — build with `cargo build --bin bobe-copilot-hook` \
             (the spike resolves the binary as a sibling of the running daemon)",
            hook_binary.display()
        );
    }

    let data_dir = crate::util::paths::bobe_data_dir();
    let worker_dir: PathBuf = data_dir.join("workers").join("spike-goals");
    std::fs::create_dir_all(&worker_dir)?;

    let mux = Arc::new(Mux::new(Duration::from_millis(120)));

    let cfg = WorkerConfig {
        name: "bobe-spike-goals".to_string(),
        dir: worker_dir.clone(),
        hook_binary,
        copilot_binary: "copilot".to_string(),
        turn_timeout: Duration::from_mins(3),
    };

    tracing::info!(dir = %worker_dir.display(), "starting spike worker");
    let worker = CopilotWorker::start(cfg, mux).await?;

    let job = JobInput {
        job_id: Uuid::new_v4(),
        kind: "goal_extraction".to_string(),
        instructions: "Extract goals from `input.text`. \
             Return JSON shaped like {\"goals\": [{\"title\": \"...\", \"why\": \"...\"}, ...]}. \
             Aim for 1-3 goals."
            .to_string(),
        input: json!({
            "text": "I want to ship the BoBe v1 release this quarter. \
                I keep procrastinating on the build pipeline. \
                Tomorrow I should make tea instead of coffee."
        }),
    };

    tracing::info!(job_id = %job.job_id, "submitting job");
    let out = worker.submit(job).await?;
    tracing::info!(job_id = %out.job_id, output = %out.output, "spike round-trip OK");

    println!("\n--- spike result ---");
    println!("job_id: {}", out.job_id);
    println!("output: {}", serde_json::to_string_pretty(&out.output)?);
    if let Some(err) = out.error {
        println!("error:  {err}");
    }

    // Leave the tmux session alive — operator can attach with `tmux a -t
    // bobe-spike-goals` to inspect. Re-runs are idempotent.
    Ok(())
}

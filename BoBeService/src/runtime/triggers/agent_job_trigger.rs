//! Monitors agent jobs, notifies on completion. Event-driven + polling fallback.

use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{info, warn};

use uuid::Uuid;

use crate::config::Config;
use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::JobInput;
use crate::db::AgentJobRepository;
use crate::models::agent_job::AgentJob;
use crate::models::types::AgentJobStatus;
use crate::runtime::proactive_generator::ProactiveGenerator;
use crate::runtime::prompts::agent_job_evaluation::AgentJobEvaluationPrompt;
use crate::runtime::state::Decision;
use crate::services::agent_job_manager::AgentJobManager;

const MAX_CONTINUATIONS: i32 = 3;

pub(crate) struct AgentJobTrigger {
    manager: Arc<AgentJobManager>,
    agent_job_repo: Arc<dyn AgentJobRepository>,
    generator: Arc<ProactiveGenerator>,
    config: Arc<ArcSwap<Config>>,
    workers: Arc<WorkerRegistry>,
}

impl AgentJobTrigger {
    pub(crate) fn new(
        manager: Arc<AgentJobManager>,
        agent_job_repo: Arc<dyn AgentJobRepository>,
        generator: Arc<ProactiveGenerator>,
        config: Arc<ArcSwap<Config>>,
        workers: Arc<WorkerRegistry>,
    ) -> Self {
        Self {
            manager,
            agent_job_repo,
            generator,
            config,
            workers,
        }
    }

    /// Must be called after wrapping `self` in `Arc`.
    pub(crate) async fn register_callback(self: &Arc<Self>) {
        let trigger = Arc::clone(self);
        let callback = Arc::new(move |job: AgentJob| {
            let trigger = Arc::clone(&trigger);
            Box::pin(async move {
                trigger.on_job_complete(job).await;
            }) as futures::future::BoxFuture<'static, ()>
        });
        self.manager.set_on_job_complete(callback).await;
        info!("agent_job_trigger.callback_registered");
    }

    async fn on_job_complete(&self, job: AgentJob) {
        let should_continue = self.should_continue(&job).await;
        if should_continue {
            self.continue_agent(&job).await;
        } else {
            self.notify_job(&job).await;
            if let Err(e) = self.agent_job_repo.mark_reported(job.id).await {
                warn!(error = %e, "agent_job_trigger.mark_reported_failed");
            }
        }
    }

    /// Polling fallback for unreported jobs that slipped through the callback.
    pub(crate) async fn fire(&self) -> Decision {
        let unreported = match self.agent_job_repo.find_unreported_terminal().await {
            Ok(jobs) => jobs,
            Err(e) => {
                warn!(error = %e, "agent_job_trigger.poll_failed");
                return Decision::Idle;
            }
        };

        if unreported.is_empty() {
            return Decision::Idle;
        }

        for job in &unreported {
            let should_continue = self.should_continue(job).await;
            if should_continue {
                self.continue_agent(job).await;
            } else {
                self.notify_job(job).await;
                if let Err(e) = self.agent_job_repo.mark_reported(job.id).await {
                    warn!(error = %e, "agent_job_trigger.mark_reported_failed");
                }
            }
        }

        Decision::Engage
    }

    async fn should_continue(&self, job: &AgentJob) -> bool {
        if job.status == AgentJobStatus::Cancelled {
            return false;
        }
        if job.continuation_count >= MAX_CONTINUATIONS {
            info!(
                job_id = %job.id,
                count = job.continuation_count,
                "agent_job.max_continuations"
            );
            return false;
        }
        if job.result_summary.is_none() && job.error_message.is_none() {
            return false;
        }

        let locale = self.config.load().effective_locale();
        let messages = AgentJobEvaluationPrompt::messages(
            &job.user_intent,
            job.result_summary.as_deref().unwrap_or(""),
            job.error_message.as_deref(),
            job.continuation_count as u32,
            Some(&locale),
        );
        // Concatenate the SDK-pre-prompt-style message list into one
        // body — BatchWorker takes a single instructions string (not
        // role-tagged messages) and the underlying autopilot turn
        // builds its own system message via memory.md context anyway.
        let prompt_body = messages
            .iter()
            .map(|m| m.content.text_or_empty().to_string())
            .collect::<Vec<_>>()
            .join("\n\n");

        let worker = match self.workers.goals().await {
            Ok(w) => w,
            Err(e) => {
                warn!(err = %e, job_id = %job.id, "agent_job.evaluation.worker_unavailable");
                return false;
            }
        };

        let job_input = JobInput {
            job_id: Uuid::new_v4(),
            kind: "agent_job_evaluation".into(),
            instructions: format!(
                "{prompt_body}\n\n\
                 Return JSON {{\"output\": \"CONTINUE\"}} or {{\"output\": \"STOP\"}} — \
                 nothing else."
            ),
            input: serde_json::Value::Null,
        };

        match worker.submit(job_input).await {
            Ok(out) => {
                let verdict = out
                    .output
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_uppercase();
                info!(
                    job_id = %job.id,
                    verdict = %verdict,
                    "agent_job.evaluation"
                );
                verdict == "CONTINUE"
            }
            Err(e) => {
                warn!(error = %e, job_id = %job.id, "agent_job.evaluation_failed");
                false
            }
        }
    }

    async fn continue_agent(&self, job: &AgentJob) {
        let continuation_prompt = format!(
            "Continue working on the original task: {}\n\n\
             Previous result: {}\n\
             Please continue until the task is fully complete.",
            job.user_intent,
            job.result_summary.as_deref().unwrap_or("No output."),
        );

        info!(
            profile = %job.profile_name,
            attempt = job.continuation_count + 1,
            "agent_job.continuing"
        );

        if let Err(e) = self.agent_job_repo.mark_reported(job.id).await {
            warn!(error = %e, "agent_job_trigger.mark_reported_failed");
        }

        match self
            .manager
            .launch(
                &job.profile_name,
                &continuation_prompt,
                Some(&job.working_directory),
                job.conversation_id,
            )
            .await
        {
            Ok(mut new_job) => {
                new_job.continuation_count = job.continuation_count + 1;
                if let Err(e) = self.agent_job_repo.save(&new_job).await {
                    warn!(error = %e, "agent_job.continuation_count_update_failed");
                }
            }
            Err(e) => {
                warn!(error = %e, "agent_job.continue_failed");
                self.notify_job(job).await;
            }
        }
    }

    async fn notify_job(&self, job: &AgentJob) {
        let cfg = self.config.load();
        let status_word = if job.status == AgentJobStatus::Completed {
            "completed"
        } else {
            "failed"
        };
        let mut parts = vec![format!(
            "Coding agent '{}' {status_word}.",
            job.profile_name
        )];

        if let Some(ref summary) = job.result_summary {
            let s = if summary.len() > 500 {
                format!("{}...", crate::util::text::truncate_str(summary, 500))
            } else {
                summary.clone()
            };
            parts.push(format!("Summary: {s}"));
        }

        if let Some(ref err) = job.error_message {
            parts.push(format!("Error: {err}"));
        }

        if let Some(cost) = job.cost_usd {
            parts.push(format!("Cost: ${cost:.4}"));
        }

        if let Some(runtime) = job.runtime_seconds() {
            parts.push(format!("Runtime: {runtime:.0}s"));
        }

        if job.continuation_count > 0 {
            parts.push(format!("Attempts: {}", job.continuation_count + 1));
        }

        let context_summary = format!(
            "Coding agent job completed. Here are the results:\n\n{}",
            parts.join(" ")
        );

        info!(profile = %job.profile_name, "agent_job.notifying");

        self.generator
            .generate_proactive_response(
                cfg.conversation.auto_close_minutes as i64,
                Some(context_summary),
            )
            .await;
    }
}

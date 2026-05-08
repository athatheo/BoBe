//! `SessionHooks` impl that composes BoBe's lifecycle behavior:
//!
//! * **`SessionStart`** — inject `memory.md` body as `additional_context`
//!   so every Copilot turn sees BoBe's current pruned memory at the
//!   start of the system message.
//! * **`UserPromptSubmitted`** — inject per-turn fresh context (current
//!   local time, recent capture timestamp). Cheap; runs every turn.
//! * **`ErrorOccurred`** — structured logging via `tracing` so daemon
//!   logs surface CLI-side errors with context.
//!
//! Each session takes one `Arc<dyn SessionHooks>`, so we compose all
//! concerns into a single impl rather than registering separate ones.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Local};
use github_copilot_sdk::hooks::{
    HookEvent, HookOutput, SessionHooks, SessionStartOutput, UserPromptSubmittedOutput,
};

use super::memory_file::MemoryFile;
use super::types::WorkerClass;

pub(crate) struct BobeHooks {
    class: WorkerClass,
    memory_file: Arc<MemoryFile>,
}

impl BobeHooks {
    pub(crate) fn new(class: WorkerClass, memory_file: Arc<MemoryFile>) -> Arc<Self> {
        Arc::new(Self { class, memory_file })
    }
}

#[async_trait]
impl SessionHooks for BobeHooks {
    async fn on_hook(&self, event: HookEvent) -> HookOutput {
        match event {
            HookEvent::SessionStart { ctx, .. } => match self.memory_file.read().await {
                Ok(body) => {
                    tracing::debug!(
                        class = %self.class.name(),
                        session = %ctx.session_id,
                        bytes = body.len(),
                        "injecting memory.md as session context"
                    );
                    HookOutput::SessionStart(SessionStartOutput {
                        additional_context: Some(body),
                        ..Default::default()
                    })
                }
                Err(e) => {
                    tracing::warn!(
                        class = %self.class.name(),
                        err = %e,
                        "memory.md read failed; session starts without context"
                    );
                    HookOutput::None
                }
            },

            HookEvent::UserPromptSubmitted { .. } => {
                let now: DateTime<Local> = Local::now();
                let context = format!(
                    "Current local time: {}. Worker class: {}.",
                    now.format("%Y-%m-%d %H:%M:%S %z"),
                    self.class.name(),
                );
                HookOutput::UserPromptSubmitted(UserPromptSubmittedOutput {
                    additional_context: Some(context),
                    ..Default::default()
                })
            }

            HookEvent::ErrorOccurred { input, ctx } => {
                tracing::warn!(
                    class = %self.class.name(),
                    session = %ctx.session_id,
                    error_context = %input.error_context,
                    recoverable = input.recoverable,
                    err = %input.error,
                    "copilot session error_occurred hook"
                );
                HookOutput::None
            }

            _ => HookOutput::None,
        }
    }
}

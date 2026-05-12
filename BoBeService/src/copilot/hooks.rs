use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Local};
use github_copilot_sdk::hooks::{
    HookEvent, HookOutput, PostToolUseOutput, SessionHooks, SessionStartOutput,
    UserPromptSubmittedOutput,
};

use super::memory_file::MemoryFile;
use super::types::WorkerClass;

/// Voice tone hint appended to UserPromptSubmitted context when the current
/// turn originated from the voice WS handler. Source: LiveKit "Prompting
/// Voice Agents to Sound More Realistic" template, trimmed for brevity.
const VOICE_TONE_HINT: &str = concat!(
    "[voice channel] Respond in 1-2 short sentences. ",
    "No markdown lists, code blocks, or headers. ",
    "Use natural speech rhythms — short clauses, conversational tone. ",
    "Spell out abbreviations as words. ",
    "If you can't help, suggest an alternative instead of refusing.",
);

/// Tool results above this serialized-char length get truncated for voice
/// turns — reading a 5kB file dump aloud is a UX disaster. ~800 chars ≈ 200
/// tokens, which is the rule-of-thumb summary threshold used in the
/// Anthropic cookbook PostToolUse pattern.
const VOICE_TOOL_RESULT_TRUNCATE_CHARS: usize = 800;

pub(crate) struct BobeHooks {
    class: WorkerClass,
    memory_file: Arc<MemoryFile>,
    /// Flipped by the voice handler around `session.send`. Hooks branch off it
    /// without per-message metadata support (Copilot SDK 0.1 has none).
    voice_turn_active: Arc<AtomicBool>,
}

impl BobeHooks {
    pub(crate) fn new(
        class: WorkerClass,
        memory_file: Arc<MemoryFile>,
        voice_turn_active: Arc<AtomicBool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            class,
            memory_file,
            voice_turn_active,
        })
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
                let mut context = format!(
                    "Current local time: {}. Worker class: {}.",
                    now.format("%Y-%m-%d %H:%M:%S %z"),
                    self.class.name(),
                );
                // Voice-tone hint goes AFTER the time/class context so it
                // sits closer to the user message (LLMs weight recent
                // instructions more strongly).
                if self.voice_turn_active.load(Ordering::Acquire) {
                    context.push_str("\n\n");
                    context.push_str(VOICE_TONE_HINT);
                }
                HookOutput::UserPromptSubmitted(UserPromptSubmittedOutput {
                    additional_context: Some(context),
                    ..Default::default()
                })
            }

            HookEvent::PostToolUse { input, ctx } => {
                // For voice turns: replace long tool results with a short
                // marker so Kokoro doesn't read file dumps / search-result
                // walls aloud. Text turns get the original result.
                if !self.voice_turn_active.load(Ordering::Acquire) {
                    return HookOutput::None;
                }
                let result_str = input.tool_result.to_string();
                if result_str.len() <= VOICE_TOOL_RESULT_TRUNCATE_CHARS {
                    return HookOutput::None;
                }
                let head: String = result_str
                    .chars()
                    .take(VOICE_TOOL_RESULT_TRUNCATE_CHARS)
                    .collect();
                tracing::debug!(
                    session = %ctx.session_id,
                    tool = %input.tool_name,
                    original_chars = result_str.len(),
                    "voice.post_tool_summary"
                );
                HookOutput::PostToolUse(PostToolUseOutput {
                    modified_result: Some(serde_json::json!({
                        "summary": format!("{head}…"),
                        "truncated_for_voice": true,
                        "original_chars": result_str.len(),
                    })),
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

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use async_trait::async_trait;
use chrono::{DateTime, Local};
use github_copilot_sdk::hooks::{
    HookEvent, HookOutput, PostToolUseOutput, PreToolUseOutput, SessionHooks, SessionStartOutput,
    UserPromptSubmittedOutput,
};

use super::memory_file::MemoryFile;
use super::types::WorkerClass;
use crate::voice::engines::VoiceEnginesSnapshot;
use crate::voice::filler_library::FillerKind;
use crate::voice::sinks::{VoiceSink, emit_filler, filler_for_tool};

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

/// If a second PreToolUse fires within this window of the first, switch
/// the second tool's filler from per-tool to compound ("Looking into a
/// few things") so two parallel tools don't read like a recipe.
const COMPOUND_FILLER_WINDOW: Duration = Duration::from_millis(100);

/// Per-WorkerRegistry voice deps that BobeHooks reads on every fire.
/// Bundled so callers don't drill 3 Arc clones on every BobeHooks::new.
/// Cheap to clone (3 Arcs).
#[derive(Clone)]
pub(crate) struct HooksVoiceContext {
    /// Flipped by the voice handler around `session.send`. Hooks branch off it
    /// without per-message metadata support (Copilot SDK 0.1 has none).
    pub(crate) voice_turn_active: Arc<AtomicBool>,
    /// Active voice WS sink for PreToolUse / ErrorOccurred filler emission.
    pub(crate) voice_sink: Arc<VoiceSink>,
    /// Voice engines snapshot — `ArcSwap` so the install service can hot-swap
    /// after a successful download. Hooks read on every fire so a post-install
    /// reload picks up the new filler library without a daemon restart.
    pub(crate) voice_engines: Arc<ArcSwap<VoiceEnginesSnapshot>>,
}

pub(crate) struct BobeHooks {
    class: WorkerClass,
    memory_file: Arc<MemoryFile>,
    voice: HooksVoiceContext,
    /// Monotonic Instant of the most recent PreToolUse fire (None = never).
    /// Used to debounce parallel tool calls into a compound filler so two
    /// tools firing within COMPOUND_FILLER_WINDOW get one "Looking into a
    /// few things" instead of stacked per-tool phrases. Mutex avoids the
    /// AtomicU64-wall-clock pitfall (NTP jump = false burst).
    last_pretool_at: Mutex<Option<Instant>>,
}

impl BobeHooks {
    pub(crate) fn new(
        class: WorkerClass,
        memory_file: Arc<MemoryFile>,
        voice: HooksVoiceContext,
    ) -> Arc<Self> {
        Arc::new(Self {
            class,
            memory_file,
            voice,
            last_pretool_at: Mutex::new(None),
        })
    }

    fn is_voice_turn(&self) -> bool {
        self.voice.voice_turn_active.load(Ordering::Acquire)
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
                if self.voice.voice_turn_active.load(Ordering::Acquire) {
                    context.push_str("\n\n");
                    context.push_str(VOICE_TONE_HINT);
                }
                HookOutput::UserPromptSubmitted(UserPromptSubmittedOutput {
                    additional_context: Some(context),
                    ..Default::default()
                })
            }

            HookEvent::PreToolUse { input, ctx } => {
                if !self.is_voice_turn() {
                    return HookOutput::None;
                }
                // ask_user blocks the LLM waiting on input — voice has no UI
                // surface for that question. Deny it cleanly so the agent
                // proceeds with a different plan instead of hanging.
                if input.tool_name == "ask_user" {
                    metrics::counter!(crate::voice::telemetry::CTR_ASK_USER_BLOCKED).increment(1);
                    tracing::info!(
                        session = %ctx.session_id,
                        "voice.pre_tool_blocked_ask_user"
                    );
                    return HookOutput::PreToolUse(PreToolUseOutput {
                        permission_decision: Some("deny".into()),
                        permission_decision_reason: Some(
                            "voice mode cannot collect user input; respond inline instead"
                                .into(),
                        ),
                        ..Default::default()
                    });
                }
                // Per-tool filler: emit cached PCM via the active voice sink
                // so the user hears "Let me search the web…" / "One sec…"
                // within ~50ms of the tool call starting. If a second
                // PreToolUse fires within COMPOUND_FILLER_WINDOW_MS the
                // second emission becomes ToolGeneric ("Looking into a few
                // things") instead of stacking another per-tool phrase.
                if let Some(library) = self.voice.voice_engines.load().filler_library.as_ref() {
                    let now = Instant::now();
                    let in_burst = {
                        let mut slot = self
                            .last_pretool_at
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let burst = slot.is_some_and(|prev| {
                            now.saturating_duration_since(prev) < COMPOUND_FILLER_WINDOW
                        });
                        *slot = Some(now);
                        burst
                    };
                    let kind = if in_burst {
                        FillerKind::ToolGeneric
                    } else {
                        filler_for_tool(&input.tool_name)
                    };
                    tracing::debug!(
                        session = %ctx.session_id,
                        tool = %input.tool_name,
                        ?kind,
                        in_burst,
                        "voice.pre_tool_filler"
                    );
                    emit_filler(&self.voice.voice_sink, library, kind).await;
                }
                HookOutput::None
            }

            HookEvent::PostToolUse { input, ctx } => {
                // For voice turns: replace long tool results with a short
                // marker so Kokoro doesn't read file dumps / search-result
                // walls aloud. Text turns get the original result.
                if !self.voice.voice_turn_active.load(Ordering::Acquire) {
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
                // Include tool_name + the original `success`-shaped fields
                // when present so the LLM keeps semantic context past the
                // truncation boundary; mid-JSON cut inside `head` is
                // expected.
                let success = input
                    .tool_result
                    .get("success")
                    .and_then(serde_json::Value::as_bool);
                HookOutput::PostToolUse(PostToolUseOutput {
                    modified_result: Some(serde_json::json!({
                        "tool_name": input.tool_name,
                        "summary": format!("{head}…"),
                        "success": success,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use github_copilot_sdk::hooks::{HookContext, UserPromptSubmittedInput};
    use github_copilot_sdk::types::SessionId;
    use std::path::PathBuf;

    fn user_prompt_event() -> HookEvent {
        HookEvent::UserPromptSubmitted {
            input: UserPromptSubmittedInput {
                timestamp: 0,
                cwd: PathBuf::from("/tmp"),
                prompt: "hello".into(),
            },
            ctx: HookContext {
                session_id: SessionId::new("test-session"),
            },
        }
    }

    fn build_hooks(voice_active: bool) -> (Arc<BobeHooks>, Arc<AtomicBool>) {
        let memory_file = MemoryFile::new(PathBuf::from("/tmp/bobe-test-memory.md"));
        let flag = Arc::new(AtomicBool::new(voice_active));
        let voice = HooksVoiceContext {
            voice_turn_active: Arc::clone(&flag),
            voice_sink: Arc::new(VoiceSink::new()),
            voice_engines: Arc::new(ArcSwap::from_pointee(VoiceEnginesSnapshot::default())),
        };
        let hooks = BobeHooks::new(WorkerClass::Chat, memory_file, voice);
        (hooks, flag)
    }

    #[tokio::test]
    async fn voice_tone_appended_when_flag_set() {
        let (hooks, _) = build_hooks(true);
        let output = hooks.on_hook(user_prompt_event()).await;
        match output {
            HookOutput::UserPromptSubmitted(UserPromptSubmittedOutput {
                additional_context: Some(ctx),
                ..
            }) => {
                assert!(
                    ctx.contains(VOICE_TONE_HINT),
                    "expected voice tone hint in context: {ctx}"
                );
            }
            other => panic!("expected UserPromptSubmitted output, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn voice_tone_absent_when_flag_clear() {
        let (hooks, _) = build_hooks(false);
        let output = hooks.on_hook(user_prompt_event()).await;
        match output {
            HookOutput::UserPromptSubmitted(UserPromptSubmittedOutput {
                additional_context: Some(ctx),
                ..
            }) => {
                assert!(
                    !ctx.contains(VOICE_TONE_HINT),
                    "voice tone hint must not leak into text turns: {ctx}"
                );
                // Sanity — the time/class context is still emitted.
                assert!(ctx.contains("Worker class:"));
            }
            other => panic!("expected UserPromptSubmitted output, got {other:?}"),
        }
    }
}

//! Copilot CLI workers built on `github-copilot-sdk`.
//!
//! Architecture (mirrors `crate::llm` shape):
//!
//! ```text
//!   types.rs           — data shapes (WorkerClass, JobInput, ChatDelta, ...)
//!   error.rs           — WorkerError
//!   client.rs          — ClientHandle: shared CLI process
//!   session_store.rs   — persistent session IDs + daily rotation
//!   memory_file.rs     — single-writer to ~/.bobe/memory.md
//!   handler.rs         — BobeHandler (permission auto-approve, usage observation)
//!   hooks.rs           — BobeHooks (memory injection, error logging, per-turn ctx)
//!   usage.rs           — UsageMeter (cost/token rollup per class)
//!   registry.rs        — WorkerRegistry: per-class lazy session spawn + shutdown
//!   consolidation.rs   — nightly memory.md prune trigger
//!   spike.rs           — `bobe spike-copilot` demo subcommand
//!   workers/
//!     batch.rs         — BatchWorker (autopilot, send_and_wait, JSON output)
//!     chat.rs          — CopilotChatWorker (interactive, streaming)
//!     vision.rs        — VisionWorker (image attachment, plain text answer)
//! ```
//!
//! Memory.md injection happens via `hooks::BobeHooks::on_session_start`
//! returning the current pruned memory as `additional_context` —
//! workers see it at the start of every turn without symlink/file-watch
//! plumbing. `~/.bobe/skills/<class>/SKILL.md` is loaded via
//! `SessionConfig::skill_directories` for stable per-class identity.

pub(crate) mod client;
pub(crate) mod consolidation;
pub(crate) mod error;
pub(crate) mod handler;
pub(crate) mod hooks;
pub(crate) mod memory_file;
pub(crate) mod registry;
pub(crate) mod session_store;
pub(crate) mod spike;
pub(crate) mod types;
pub(crate) mod usage;
pub(crate) mod workers;

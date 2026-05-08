//! Copilot CLI workers: per-class tmux session running `copilot`,
//! file-based inbox/outbox, hook-driven completion signals.
//!
//! See `~/.claude/projects/-Users-john-Repos-bobrust/memory/project_copilot_workers_initiative.md`
//! for the active design and phasing. Patterns adapted from `tebis`
//! (`platform::multiplexer`, `platform::peer_listener`,
//! `agent_hooks::copilot`) and `Kodosi` (versioned hook envelope).

pub(crate) mod hook;
pub(crate) mod hook_install;
pub(crate) mod mux;
pub(crate) mod spike;
pub(crate) mod worker;

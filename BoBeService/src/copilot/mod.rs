//! Copilot CLI workers: per-class SDK session backed by `github-copilot-sdk`.
//! One shared `Client` (== one Copilot CLI server process) owns N `Session`s,
//! one per worker class (goals/observe/vision/chat/consolidate).
//!
//! Memory injection happens via the `on_session_start` hook returning the
//! current `memory.md` body as `additional_context` — workers see pruned
//! memory at the start of every turn without symlink/file-watching games.
//!
//! See `~/.claude/projects/-Users-john-Repos-bobrust/memory/project_copilot_workers_initiative.md`
//! for the active design and phasing.

pub(crate) mod agent_worker;
pub(crate) mod classes;
pub(crate) mod consolidation;
pub(crate) mod memory_file;
pub(crate) mod registry;
pub(crate) mod spike;
pub(crate) mod worker;

//! Copilot CLI workers built on `github-copilot-sdk`. One shared
//! `Client` per daemon, one `Session` per `WorkerClass`. Memory.md is
//! injected at `SessionStart` via `hooks::BobeHooks`; skills load from
//! `~/.bobe/skills/<class>/SKILL.md` via `SessionConfig::skill_directories`.

pub(crate) mod client;
pub(crate) mod consolidation;
pub(crate) mod error;
pub(crate) mod handler;
pub(crate) mod hooks;
pub(crate) mod memory_file;
pub(crate) mod registry;
pub(crate) mod session_store;
pub(crate) mod skills;
pub(crate) mod types;
pub(crate) mod usage;
pub(crate) mod workers;

//! Ollama install + lifecycle cluster — the daemon downloads + extracts +
//! launches a managed Ollama process to serve local LLM requests when the
//! engine is set to `local`. Layout:
//!
//! - `manager` — child-process spawn + health/teardown
//! - `binary_manager` — discover or download the Ollama tarball
//! - `binary_download` / `binary_extract` — submodules of binary_manager
//! - `install_service` — the public service exposed via `AppState`

pub(crate) mod binary_download;
pub(crate) mod binary_extract;
pub(crate) mod binary_manager;
pub(crate) mod install_service;
pub(crate) mod manager;

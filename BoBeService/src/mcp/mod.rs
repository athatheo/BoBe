//! MCP config schema parsing + secret resolution + safety validation.
//! Reads `~/.bobe/mcp.json` and produces `HashMap<String, McpServerConfig>`
//! that `WorkerRegistry` hands to the SDK via `SessionConfig::mcp_servers`.
//! The SDK owns server lifecycle and tool dispatch — this module is
//! config-only.

pub(crate) mod config;
pub(crate) mod security;

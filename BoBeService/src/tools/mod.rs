//! MCP server config + lifecycle. Phase 5-mcp will fold MCP server
//! registration into the Copilot SDK via `SessionConfig::mcp_servers`;
//! this module survives until then to keep `/api/tools/mcp/config`
//! operational.

pub(crate) mod mcp;

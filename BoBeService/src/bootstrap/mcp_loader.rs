//! MCP server config loading for the SDK. Resolved at boot from the
//! user's MCP config file; passed to the SDK's `SessionConfig::mcp_servers`.

use std::collections::HashMap;

use tracing::{info, warn};

use crate::config::Config;

pub(super) fn load_mcp_servers_for_sdk(
    config: &Config,
) -> HashMap<String, github_copilot_sdk::types::McpServerConfig> {
    if !config.mcp.enabled {
        return HashMap::new();
    }

    let path = match crate::mcp::config::resolve_mcp_config_path(config.mcp.config_file.as_deref())
    {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "bootstrap.mcp_config_path_resolution_failed");
            return HashMap::new();
        }
    };

    let parsed = match crate::mcp::config::load_mcp_config(
        &path,
        &config.mcp.blocked_commands,
        &config.mcp.dangerous_env_keys,
    ) {
        Ok(servers) => servers,
        Err(e) => {
            warn!(error = %e, path = %path.display(), "bootstrap.mcp_config_parse_failed");
            return HashMap::new();
        }
    };

    let count = parsed.len();
    let map = crate::mcp::config::to_sdk_mcp_servers(parsed);
    info!(count, "bootstrap.mcp_servers_registered_via_sdk");
    map
}

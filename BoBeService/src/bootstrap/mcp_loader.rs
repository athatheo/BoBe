//! MCP server config loading for the SDK. Resolved at boot from the
//! user's MCP config file; passed to the SDK's `SessionConfig::mcp_servers`.

use github_copilot_sdk::IndexMap;
use tracing::{info, warn};

use crate::config::Config;

pub(crate) struct LoadedMcpServers {
    pub(crate) servers: IndexMap<String, github_copilot_sdk::types::McpServerConfig>,
    pub(crate) excluded_tools: Vec<String>,
}

pub(crate) fn load_mcp_servers_for_sdk(config: &Config) -> LoadedMcpServers {
    if !config.mcp.enabled {
        return LoadedMcpServers {
            servers: IndexMap::new(),
            excluded_tools: Vec::new(),
        };
    }

    let path = match crate::mcp::config::resolve_mcp_config_path(
        std::path::Path::new(&config.data_dir),
        config.mcp.config_file.as_deref(),
    ) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "bootstrap.mcp_config_path_resolution_failed");
            return LoadedMcpServers {
                servers: IndexMap::new(),
                excluded_tools: Vec::new(),
            };
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
            return LoadedMcpServers {
                servers: IndexMap::new(),
                excluded_tools: Vec::new(),
            };
        }
    };

    let count = parsed.len();
    let (servers, excluded_tools) = crate::mcp::config::to_sdk_mcp_servers(parsed);
    info!(count, "bootstrap.mcp_servers_registered_via_sdk");
    LoadedMcpServers {
        servers,
        excluded_tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mcp_produces_an_empty_runtime_map() {
        let mut config = Config::default();
        config.mcp.enabled = false;
        config.mcp.config_file = Some("/path/that/must/not/be/read.json".to_owned());

        let loaded = load_mcp_servers_for_sdk(&config);

        assert!(loaded.servers.is_empty());
        assert!(loaded.excluded_tools.is_empty());
    }
}

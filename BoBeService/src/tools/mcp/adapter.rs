use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info, warn};

use super::client::{McpClient, McpToolInfo};
use super::config::{McpParsedServer, load_mcp_config};
use crate::error::AppError;

const TOOL_NAME_SEPARATOR: &str = "__";

pub(crate) struct McpToolAdapter {
    clients: DashMap<String, Arc<McpClient>>,
    tool_to_server: DashMap<String, String>,
    server_configs: DashMap<String, McpParsedServer>,
    config_path: PathBuf,
    blocked_commands: Vec<String>,
    dangerous_env_keys: Vec<String>,
}

impl McpToolAdapter {
    pub(crate) fn new(
        config_path: PathBuf,
        blocked_commands: Vec<String>,
        dangerous_env_keys: Vec<String>,
    ) -> Self {
        Self {
            clients: DashMap::new(),
            tool_to_server: DashMap::new(),
            server_configs: DashMap::new(),
            config_path,
            blocked_commands,
            dangerous_env_keys,
        }
    }

    pub(crate) async fn initialize(&self) -> Result<(), AppError> {
        let servers = self.load_enabled_servers().await;
        if servers.is_empty() {
            debug!("No MCP servers configured");
            return Ok(());
        }

        info!(count = servers.len(), "Initializing MCP servers");
        for server in servers {
            if let Err(e) = self.connect_server(server).await {
                warn!(error = %e, "Failed to connect MCP server");
            }
        }
        Ok(())
    }

    pub(crate) async fn shutdown(&self) {
        let snapshot: Vec<(String, Arc<McpClient>)> = self
            .clients
            .iter()
            .map(|e| (e.key().clone(), Arc::clone(e.value())))
            .collect();

        for (name, client) in &snapshot {
            info!(server = %name, "Disconnecting MCP server");
            client.disconnect().await;
        }
        self.clients.clear();
        self.tool_to_server.clear();
        self.server_configs.clear();
    }

    pub(crate) async fn reload_from_config(&self) -> Result<(), AppError> {
        self.shutdown().await;
        self.initialize().await
    }

    pub(crate) async fn get_server_error(&self, name: &str) -> Option<String> {
        let client = self.clients.get(name)?;
        client.value().last_error().await
    }

    pub(crate) async fn get_raw_tools_for_server(
        &self,
        server_name: &str,
    ) -> Result<Vec<McpToolInfo>, AppError> {
        let client = self
            .clients
            .get(server_name)
            .ok_or_else(|| AppError::Mcp(format!("Server '{server_name}' not found")))?;

        if !client.is_connected() {
            return Err(AppError::Mcp(format!(
                "Server '{server_name}' is not connected"
            )));
        }

        client.list_tools().await
    }

    async fn load_enabled_servers(&self) -> Vec<McpParsedServer> {
        if !self.config_path.exists() {
            return Vec::new();
        }

        match load_mcp_config(
            &self.config_path,
            &self.blocked_commands,
            &self.dangerous_env_keys,
        ) {
            Ok(servers) => servers,
            Err(e) => {
                warn!(
                    error = %e,
                    path = %self.config_path.display(),
                    "mcp.load_enabled_servers_failed"
                );
                Vec::new()
            }
        }
    }

    async fn connect_server(&self, config: McpParsedServer) -> Result<(), AppError> {
        let name = config.name.clone();
        info!(server = %name, "Connecting MCP server");

        let client = Arc::new(McpClient::new(config.clone()));
        client.connect().await?;

        let tools = client.list_tools().await?;
        for tool in &tools {
            if !config.excluded_tools.contains(&tool.name) {
                self.tool_to_server
                    .insert(prefix_tool_name(&name, &tool.name), name.clone());
            }
        }

        self.clients.insert(name.clone(), client);
        self.server_configs.insert(name.clone(), config);

        info!(server = %name, tool_count = tools.len(), "MCP server connected");
        Ok(())
    }
}

// Tool dispatch is owned by the Copilot SDK's session loop now —
// MCP servers get registered via `SessionConfig::mcp_servers` and
// the SDK manages process lifecycle + tool calls. McpToolAdapter
// retains lifecycle (initialize/shutdown/reload) for the
// `/api/tools/mcp/config` UI's runtime state queries; the
// dispatch path is gone.

fn prefix_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{server_name}{TOOL_NAME_SEPARATOR}{tool_name}")
}

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info, warn};

use super::client::{McpClient, McpToolInfo};
use super::config::{McpParsedServer, load_mcp_config};
use crate::error::AppError;
use crate::llm::types::{AiToolCall, ToolDefinition};
use crate::tools::{ToolExecutionContext, ToolResult, ToolSource};

const TOOL_NAME_SEPARATOR: &str = "__";

/// Manages multiple MCP server connections and exposes their tools.
///
/// Uses `DashMap` for lock-free concurrent access to client/tool/config maps.
pub struct McpToolAdapter {
    clients: DashMap<String, Arc<McpClient>>,
    tool_to_server: DashMap<String, String>,
    server_configs: DashMap<String, McpParsedServer>,
    config_path: PathBuf,
    blocked_commands: Vec<String>,
    dangerous_env_keys: Vec<String>,
}

impl McpToolAdapter {
    pub fn new(
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

    /// Initialize all MCP servers from file config.
    pub async fn initialize(&self) -> Result<(), AppError> {
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

    /// Shut down all MCP server connections.
    pub async fn shutdown(&self) {
        let snapshot: Vec<(String, Arc<McpClient>)> = self
            .clients
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();

        for (name, client) in &snapshot {
            info!(server = %name, "Disconnecting MCP server");
            client.disconnect().await;
        }
        self.clients.clear();
        self.tool_to_server.clear();
        self.server_configs.clear();
    }

    /// Reload all MCP servers from the canonical config file.
    pub async fn reload_from_config(&self) -> Result<(), AppError> {
        self.shutdown().await;
        self.initialize().await
    }

    /// Get the last error for a specific server.
    pub async fn get_server_error(&self, name: &str) -> Option<String> {
        let client = self.clients.get(name)?;
        client.value().last_error().await
    }

    /// Get unfiltered/raw tools directly from the MCP server.
    pub async fn get_raw_tools_for_server(
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

#[async_trait]
impl ToolSource for McpToolAdapter {
    fn name(&self) -> &str {
        "mcp"
    }

    async fn get_tools(&self, _include_disabled: bool) -> Result<Vec<ToolDefinition>, AppError> {
        let mut all_defs = Vec::new();

        // Collect client refs outside the DashMap iterator to avoid !Send guards across awaits.
        let snapshot: Vec<(String, Arc<McpClient>)> = self
            .clients
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();

        for (server_name, client) in &snapshot {
            if !client.is_connected() {
                continue;
            }

            match client.list_tools().await {
                Ok(tools) => {
                    let excluded = self
                        .server_configs
                        .get(server_name)
                        .map(|e| e.excluded_tools.clone())
                        .unwrap_or_default();

                    for tool in tools {
                        if !excluded.contains(&tool.name) {
                            all_defs.push(to_tool_definition(server_name, &tool));
                        }
                    }
                }
                Err(e) => warn!(server = %server_name, error = %e, "Failed to list MCP tools"),
            }
        }

        Ok(all_defs)
    }

    async fn execute(
        &self,
        tool_call: &AiToolCall,
        _context: Option<&ToolExecutionContext>,
    ) -> ToolResult {
        let server_name = match self.tool_to_server.get(&tool_call.name) {
            Some(e) => e.value().clone(),
            None => {
                return ToolResult::err(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    format!("No MCP server found for tool '{}'", tool_call.name),
                );
            }
        };

        let client = match self.clients.get(&server_name) {
            Some(e) => e.value().clone(),
            None => {
                return ToolResult::err(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    format!("MCP server '{server_name}' not found"),
                );
            }
        };

        if !client.is_connected() {
            return ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!("MCP server '{server_name}' is not connected"),
            );
        }

        let original_name = unprefix_tool_name(&tool_call.name);
        debug!(server = %server_name, tool = %original_name, "Executing MCP tool");

        let timeout = std::time::Duration::from_secs_f64(client.timeout_seconds());
        match tokio::time::timeout(
            timeout,
            client.call_tool(&original_name, tool_call.arguments.clone()),
        )
        .await
        {
            Ok(Ok((true, content))) => {
                ToolResult::ok(tool_call.id.clone(), tool_call.name.clone(), content)
            }
            Ok(Ok((false, content))) => {
                ToolResult::err(tool_call.id.clone(), tool_call.name.clone(), content)
            }
            Ok(Err(e)) => {
                ToolResult::err(tool_call.id.clone(), tool_call.name.clone(), e.to_string())
            }
            Err(_) => ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!(
                    "MCP tool execution timed out after {:.0}s",
                    client.timeout_seconds()
                ),
            ),
        }
    }
}

fn to_tool_definition(server_name: &str, tool: &McpToolInfo) -> ToolDefinition {
    ToolDefinition {
        name: prefix_tool_name(server_name, &tool.name),
        description: format!("[MCP: {server_name}] {}", tool.description),
        parameters: tool.input_schema.clone(),
    }
}

fn prefix_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{server_name}{TOOL_NAME_SEPARATOR}{tool_name}")
}

fn unprefix_tool_name(prefixed: &str) -> String {
    if let Some(pos) = prefixed.find(TOOL_NAME_SEPARATOR) {
        prefixed[pos + TOOL_NAME_SEPARATOR.len()..].to_owned()
    } else {
        prefixed.to_owned()
    }
}

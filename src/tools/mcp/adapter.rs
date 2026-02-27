use async_trait::async_trait;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info, warn};

use super::client::{McpClient, McpToolInfo};
use super::config::{McpParsedServer, load_default_mcp_config};
use crate::db::McpConfigRepository;
use crate::error::AppError;
use crate::llm::types::{AiToolCall, ToolDefinition};
use crate::models::mcp_server_config::McpServerConfig;
use crate::tools::{ToolExecutionContext, ToolResult, ToolSource};

const TOOL_NAME_SEPARATOR: &str = "__";

/// Manages multiple MCP server connections and exposes their tools.
///
/// Uses `DashMap` for lock-free concurrent access to client/tool/config maps.
pub struct McpToolAdapter {
    clients: DashMap<String, Arc<McpClient>>,
    tool_to_server: DashMap<String, String>,
    server_configs: DashMap<String, McpParsedServer>,
    config_repo: Option<Arc<dyn McpConfigRepository>>,
    blocked_commands: Vec<String>,
    dangerous_env_keys: Vec<String>,
}

impl McpToolAdapter {
    pub fn new(
        config_repo: Option<Arc<dyn McpConfigRepository>>,
        blocked_commands: Vec<String>,
        dangerous_env_keys: Vec<String>,
    ) -> Self {
        Self {
            clients: DashMap::new(),
            tool_to_server: DashMap::new(),
            server_configs: DashMap::new(),
            config_repo,
            blocked_commands,
            dangerous_env_keys,
        }
    }

    /// Initialize all MCP servers from DB or file config.
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

    /// Reconnect a single server by name.
    pub async fn reconnect_server(&self, name: &str) -> Result<(), AppError> {
        if let Some((_, client)) = self.clients.remove(name) {
            client.disconnect().await;
        }
        self.tool_to_server.retain(|_, v| v != name);

        if let Some(config) = self.server_configs.get(name).map(|e| e.value().clone()) {
            self.connect_server(config).await?;
        }
        Ok(())
    }

    /// Add a server config and connect it at runtime.
    pub async fn add_and_connect_server(&self, config: McpParsedServer) -> Result<bool, AppError> {
        let name = config.name.clone();

        // Disconnect existing if present
        if let Some((_, client)) = self.clients.remove(&name) {
            client.disconnect().await;
        }
        self.tool_to_server.retain(|_, v| v != &name);

        match self.connect_server(config).await {
            Ok(()) => Ok(true),
            Err(e) => {
                warn!(server = %name, error = %e, "Failed to connect MCP server");
                Ok(false)
            }
        }
    }

    /// Disconnect and remove a server by name.
    pub async fn disconnect_server_by_name(&self, name: &str) -> bool {
        if let Some((_, client)) = self.clients.remove(name) {
            client.disconnect().await;
            self.tool_to_server.retain(|_, v| v != name);
            self.server_configs.remove(name);
            info!(server = %name, "MCP server disconnected");
            true
        } else {
            false
        }
    }

    /// Get the last error for a specific server.
    pub async fn get_server_error(&self, name: &str) -> Option<String> {
        let client = self.clients.get(name)?;
        client.value().last_error().await
    }

    /// Get tools for a specific server.
    pub async fn get_tools_for_server(
        &self,
        server_name: &str,
    ) -> Result<Vec<ToolDefinition>, AppError> {
        let client = self
            .clients
            .get(server_name)
            .ok_or_else(|| AppError::Mcp(format!("Server '{server_name}' not found")))?;

        if !client.is_connected() {
            return Err(AppError::Mcp(format!(
                "Server '{server_name}' is not connected"
            )));
        }

        let tools = client.list_tools().await?;
        let excluded = self
            .server_configs
            .get(server_name)
            .map(|e| e.excluded_tools.clone())
            .unwrap_or_default();

        Ok(tools
            .into_iter()
            .filter(|t| !excluded.contains(&t.name))
            .map(|t| to_tool_definition(server_name, &t))
            .collect())
    }

    async fn load_enabled_servers(&self) -> Vec<McpParsedServer> {
        if let Some(repo) = &self.config_repo
            && let Ok(configs) = repo.find_enabled().await
        {
            return configs
                .into_iter()
                .map(|c| db_config_to_parsed(&c))
                .collect();
        }
        load_default_mcp_config(&self.blocked_commands, &self.dangerous_env_keys)
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

pub fn db_config_to_parsed(c: &McpServerConfig) -> McpParsedServer {
    McpParsedServer {
        name: c.server_name.clone(),
        command: c.command.clone(),
        args: c.args_vec(),
        env: c.env_map(),
        timeout_seconds: c.timeout_seconds,
        excluded_tools: c
            .excluded_tools
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default(),
    }
}

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::client::{McpClient, McpToolInfo};
use super::config::{McpParsedServer, load_default_mcp_config};
use crate::domain::mcp_server_config::McpServerConfig;
use crate::error::AppError;
use crate::ports::llm_types::{AiToolCall, ToolDefinition};
use crate::ports::repos::mcp_config_repo::McpConfigRepository;
use crate::ports::tools::{ToolCategory, ToolExecutionContext, ToolResult, ToolSource};

const TOOL_NAME_SEPARATOR: &str = "__";

/// Manages multiple MCP server connections and exposes their tools.
pub struct McpToolAdapter {
    clients: RwLock<HashMap<String, Arc<McpClient>>>,
    tool_to_server: RwLock<HashMap<String, String>>,
    server_configs: RwLock<HashMap<String, McpParsedServer>>,
    config_repo: Option<Arc<dyn McpConfigRepository>>,
    categories: Vec<ToolCategory>,
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
            clients: RwLock::new(HashMap::new()),
            tool_to_server: RwLock::new(HashMap::new()),
            server_configs: RwLock::new(HashMap::new()),
            config_repo,
            categories: vec![ToolCategory::Mcp],
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
        let clients = self.clients.read().await;
        for (name, client) in clients.iter() {
            info!(server = %name, "Disconnecting MCP server");
            client.disconnect().await;
        }
        drop(clients);

        self.clients.write().await.clear();
        self.tool_to_server.write().await.clear();
        self.server_configs.write().await.clear();
    }

    /// Reconnect a single server by name.
    pub async fn reconnect_server(&self, name: &str) -> Result<(), AppError> {
        // Disconnect existing
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.remove(name) {
            client.disconnect().await;
        }
        drop(clients);

        // Remove tool mappings
        let mut t2s = self.tool_to_server.write().await;
        t2s.retain(|_, v| v != name);
        drop(t2s);

        // Reload config and reconnect
        let configs = self.server_configs.read().await;
        if let Some(config) = configs.get(name).cloned() {
            drop(configs);
            self.connect_server(config).await?;
        }
        Ok(())
    }

    /// Add a server config and connect it at runtime.
    pub async fn add_and_connect_server(&self, config: McpParsedServer) -> Result<bool, AppError> {
        let name = config.name.clone();

        // Disconnect existing if present
        {
            let mut clients = self.clients.write().await;
            if let Some(client) = clients.remove(&name) {
                client.disconnect().await;
            }
        }
        {
            let mut t2s = self.tool_to_server.write().await;
            t2s.retain(|_, v| v != &name);
        }

        match self.connect_server(config).await {
            Ok(_) => Ok(true),
            Err(e) => {
                warn!(server = %name, error = %e, "Failed to connect MCP server");
                Ok(false)
            }
        }
    }

    /// Disconnect and remove a server by name.
    pub async fn disconnect_server_by_name(&self, name: &str) -> bool {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.remove(name) {
            client.disconnect().await;
            drop(clients);

            let mut t2s = self.tool_to_server.write().await;
            t2s.retain(|_, v| v != name);

            self.server_configs.write().await.remove(name);

            info!(server = %name, "MCP server disconnected");
            true
        } else {
            false
        }
    }

    /// Get the last error for a specific server.
    pub async fn get_server_error(&self, name: &str) -> Option<String> {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(name) {
            client.last_error().await
        } else {
            None
        }
    }

    /// Get tools for a specific server.
    pub async fn get_tools_for_server(
        &self,
        server_name: &str,
    ) -> Result<Vec<ToolDefinition>, AppError> {
        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| AppError::Mcp(format!("Server '{server_name}' not found")))?;

        if !client.is_connected() {
            return Err(AppError::Mcp(format!(
                "Server '{server_name}' is not connected"
            )));
        }

        let tools = client.list_tools().await?;
        let configs = self.server_configs.read().await;
        let excluded = configs
            .get(server_name)
            .map(|c| &c.excluded_tools)
            .cloned()
            .unwrap_or_default();

        let defs = tools
            .into_iter()
            .filter(|t| !excluded.contains(&t.name))
            .map(|t| self.to_tool_definition(server_name, &t))
            .collect();

        Ok(defs)
    }

    async fn load_enabled_servers(&self) -> Vec<McpParsedServer> {
        // Try DB first
        if let Some(repo) = &self.config_repo
            && let Ok(configs) = repo.find_enabled().await
        {
            return configs
                .into_iter()
                .map(|c| db_config_to_parsed(&c))
                .collect();
        }
        // Fall back to file
        load_default_mcp_config(&self.blocked_commands, &self.dangerous_env_keys)
    }

    async fn connect_server(&self, config: McpParsedServer) -> Result<(), AppError> {
        let name = config.name.clone();
        info!(server = %name, "Connecting MCP server");

        let client = Arc::new(McpClient::new(config.clone()));
        client.connect().await?;

        // List tools and build mappings
        let tools = client.list_tools().await?;
        let mut t2s = self.tool_to_server.write().await;
        for tool in &tools {
            if !config.excluded_tools.contains(&tool.name) {
                let prefixed = prefix_tool_name(&name, &tool.name);
                t2s.insert(prefixed, name.clone());
            }
        }
        drop(t2s);

        self.clients.write().await.insert(name.clone(), client);
        self.server_configs
            .write()
            .await
            .insert(name.clone(), config);

        info!(server = %name, tool_count = tools.len(), "MCP server connected");
        Ok(())
    }

    fn to_tool_definition(&self, server_name: &str, tool: &McpToolInfo) -> ToolDefinition {
        ToolDefinition {
            name: prefix_tool_name(server_name, &tool.name),
            description: format!("[MCP: {}] {}", server_name, tool.description),
            parameters: tool.input_schema.clone(),
        }
    }
}

#[async_trait]
impl ToolSource for McpToolAdapter {
    fn name(&self) -> &str {
        "mcp"
    }

    fn categories(&self) -> &[ToolCategory] {
        &self.categories
    }

    async fn get_tools(&self, _include_disabled: bool) -> Result<Vec<ToolDefinition>, AppError> {
        let clients = self.clients.read().await;
        let mut all_defs = Vec::new();

        for (server_name, client) in clients.iter() {
            if !client.is_connected() {
                continue;
            }

            match client.list_tools().await {
                Ok(tools) => {
                    let configs = self.server_configs.read().await;
                    let excluded = configs
                        .get(server_name)
                        .map(|c| &c.excluded_tools)
                        .cloned()
                        .unwrap_or_default();

                    for tool in tools {
                        if !excluded.contains(&tool.name) {
                            all_defs.push(self.to_tool_definition(server_name, &tool));
                        }
                    }
                }
                Err(e) => {
                    warn!(server = %server_name, error = %e, "Failed to list MCP tools");
                }
            }
        }

        Ok(all_defs)
    }

    async fn execute(
        &self,
        tool_call: &AiToolCall,
        _context: Option<&ToolExecutionContext>,
    ) -> ToolResult {
        let t2s = self.tool_to_server.read().await;
        let server_name = match t2s.get(&tool_call.name) {
            Some(s) => s.clone(),
            None => {
                return ToolResult::err(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    format!("No MCP server found for tool '{}'", tool_call.name),
                );
            }
        };
        drop(t2s);

        let clients = self.clients.read().await;
        let client = match clients.get(&server_name) {
            Some(c) => c.clone(),
            None => {
                return ToolResult::err(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    format!("MCP server '{server_name}' not found"),
                );
            }
        };
        drop(clients);

        if !client.is_connected() {
            return ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!("MCP server '{server_name}' is not connected"),
            );
        }

        let original_name = unprefix_tool_name(&tool_call.name);
        debug!(
            server = %server_name,
            tool = %original_name,
            "Executing MCP tool"
        );

        let timeout = std::time::Duration::from_secs_f64(client.timeout_seconds());
        let result = tokio::time::timeout(
            timeout,
            client.call_tool(&original_name, tool_call.arguments.clone()),
        )
        .await;

        match result {
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

    async fn health_check(&self) -> bool {
        let clients = self.clients.read().await;
        if clients.is_empty() {
            return true;
        }
        for client in clients.values() {
            if client.health_check().await {
                return true;
            }
        }
        false
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
        enabled: c.enabled,
        timeout_seconds: c.timeout_seconds,
        excluded_tools: c
            .excluded_tools
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default(),
    }
}

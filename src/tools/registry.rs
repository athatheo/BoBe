use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::AppError;
use crate::llm::types::ToolDefinition;
use crate::tools::{ToolCategory, ToolSource};

/// Central registry that aggregates all tool sources (native + MCP).
pub struct ToolRegistry {
    sources: RwLock<HashMap<String, Arc<dyn ToolSource>>>,
    tool_to_source: RwLock<HashMap<String, String>>,
    /// Per-tool enabled/disabled overrides.
    enabled_overrides: RwLock<HashMap<String, bool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(HashMap::new()),
            tool_to_source: RwLock::new(HashMap::new()),
            enabled_overrides: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool source.
    pub async fn register(&self, source: Arc<dyn ToolSource>) {
        let name = source.name().to_owned();
        debug!(source = %name, "Registering tool source");

        // Index tool→source mappings
        if let Ok(tools) = source.get_tools(true).await {
            let mut t2s = self.tool_to_source.write().await;
            for tool in &tools {
                t2s.insert(tool.name.clone(), name.clone());
            }
        }

        self.sources.write().await.insert(name, source);
    }

    /// Unregister a tool source.
    pub async fn unregister(&self, source_name: &str) {
        self.sources.write().await.remove(source_name);
        let mut t2s = self.tool_to_source.write().await;
        t2s.retain(|_, v| v != source_name);
    }

    /// Get all available tool definitions from all sources.
    pub async fn get_all_tools(&self, include_disabled: bool) -> Vec<ToolDefinition> {
        let sources = self.sources.read().await;
        let mut all = Vec::new();

        for source in sources.values() {
            match source.get_tools(include_disabled).await {
                Ok(tools) => all.extend(tools),
                Err(e) => {
                    warn!(source = %source.name(), error = %e, "Failed to get tools from source");
                }
            }
        }
        all
    }

    /// Get tools filtered by categories.
    pub async fn get_tools_by_category(&self, categories: &[ToolCategory]) -> Vec<ToolDefinition> {
        let sources = self.sources.read().await;
        let mut result = Vec::new();

        for source in sources.values() {
            let source_cats = source.categories();
            if source_cats.iter().any(|c| categories.contains(c))
                && let Ok(tools) = source.get_tools(false).await
            {
                result.extend(tools);
            }
        }
        result
    }

    /// Find the source that provides a given tool.
    pub async fn get_source_for_tool(&self, tool_name: &str) -> Option<Arc<dyn ToolSource>> {
        let t2s = self.tool_to_source.read().await;
        let source_name = t2s.get(tool_name)?;
        let sources = self.sources.read().await;
        sources.get(source_name).cloned()
    }

    /// Get a source by name.
    pub async fn get_source(&self, name: &str) -> Option<Arc<dyn ToolSource>> {
        self.sources.read().await.get(name).cloned()
    }

    /// Get all registered source names.
    pub async fn source_names(&self) -> Vec<String> {
        self.sources.read().await.keys().cloned().collect()
    }

    /// Get the source name for a given tool.
    pub async fn get_source_name_for_tool(&self, tool_name: &str) -> Option<String> {
        self.tool_to_source.read().await.get(tool_name).cloned()
    }

    /// Health check all sources.
    pub async fn health_check_all(&self) -> HashMap<String, bool> {
        let sources = self.sources.read().await;
        let mut results = HashMap::new();
        for (name, source) in sources.iter() {
            results.insert(name.clone(), source.health_check().await);
        }
        results
    }

    /// Rebuild the tool→source index.
    pub async fn refresh_index(&self) -> Result<(), AppError> {
        let sources = self.sources.read().await;
        let mut t2s = self.tool_to_source.write().await;
        t2s.clear();

        for (source_name, source) in sources.iter() {
            if let Ok(tools) = source.get_tools(true).await {
                for tool in tools {
                    t2s.insert(tool.name, source_name.clone());
                }
            }
        }
        Ok(())
    }

    /// Check whether a tool is enabled (returns None if tool is unknown).
    pub async fn is_tool_enabled(&self, tool_name: &str) -> Option<bool> {
        let t2s = self.tool_to_source.read().await;
        if !t2s.contains_key(tool_name) {
            return None;
        }
        let overrides = self.enabled_overrides.read().await;
        Some(overrides.get(tool_name).copied().unwrap_or(true))
    }

    /// Set the enabled state of a tool. Returns `true` if the tool exists.
    pub async fn set_tool_enabled(&self, tool_name: &str, enabled: bool) -> bool {
        let t2s = self.tool_to_source.read().await;
        if !t2s.contains_key(tool_name) {
            return false;
        }
        drop(t2s);
        self.enabled_overrides
            .write()
            .await
            .insert(tool_name.to_owned(), enabled);
        debug!(tool = %tool_name, enabled, "Tool enabled state changed");
        true
    }

    /// Enable a tool. Returns `true` if the tool exists.
    pub async fn enable_tool(&self, tool_name: &str) -> bool {
        self.set_tool_enabled(tool_name, true).await
    }

    /// Disable a tool. Returns `true` if the tool exists.
    pub async fn disable_tool(&self, tool_name: &str) -> bool {
        self.set_tool_enabled(tool_name, false).await
    }
}

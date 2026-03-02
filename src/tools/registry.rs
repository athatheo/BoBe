use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, warn};

use crate::error::AppError;
use crate::llm::types::ToolDefinition;
use crate::tools::ToolSource;

/// Central registry that aggregates all tool sources (native + MCP).
///
/// Uses `DashMap` for lock-free concurrent reads on the hot path
/// (tool lookups during every LLM call) with rare writes (registration).
pub struct ToolRegistry {
    sources: DashMap<String, Arc<dyn ToolSource>>,
    tool_to_source: DashMap<String, String>,
    enabled_overrides: DashMap<String, bool>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            sources: DashMap::new(),
            tool_to_source: DashMap::new(),
            enabled_overrides: DashMap::new(),
        }
    }

    /// Register a tool source and index its tools.
    pub async fn register(&self, source: Arc<dyn ToolSource>) {
        let name = source.name().to_owned();
        debug!(source = %name, "Registering tool source");

        if let Ok(tools) = source.get_tools(true).await {
            for tool in &tools {
                self.tool_to_source.insert(tool.name.clone(), name.clone());
            }
        }

        self.sources.insert(name, source);
    }

    /// Collect all tool definitions from every registered source.
    pub async fn get_all_tools(&self, include_disabled: bool) -> Vec<ToolDefinition> {
        let sources: Vec<Arc<dyn ToolSource>> =
            self.sources.iter().map(|e| e.value().clone()).collect();

        let mut all = Vec::new();
        for source in &sources {
            match source.get_tools(include_disabled).await {
                Ok(tools) => all.extend(tools),
                Err(e) => warn!(source = %source.name(), error = %e, "Failed to get tools"),
            }
        }
        all
    }

    /// Find the source that provides a given tool.
    pub async fn get_source_for_tool(&self, tool_name: &str) -> Option<Arc<dyn ToolSource>> {
        let source_name = self.tool_to_source.get(tool_name)?;
        self.sources
            .get(source_name.value())
            .map(|e| e.value().clone())
    }

    /// Rebuild the tool→source index from all registered sources.
    pub async fn refresh_index(&self) -> Result<(), AppError> {
        self.tool_to_source.clear();
        let sources: Vec<(String, Arc<dyn ToolSource>)> = self
            .sources
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();

        for (source_name, source) in &sources {
            if let Ok(tools) = source.get_tools(true).await {
                for tool in tools {
                    self.tool_to_source.insert(tool.name, source_name.clone());
                }
            }
        }
        Ok(())
    }

    /// Check whether a tool is enabled (`None` if tool is unknown).
    pub fn is_tool_enabled(&self, tool_name: &str) -> Option<bool> {
        if !self.tool_to_source.contains_key(tool_name) {
            return None;
        }
        Some(
            self.enabled_overrides
                .get(tool_name)
                .is_none_or(|e| *e.value()),
        )
    }

    /// Set the enabled state of a tool. Returns `true` if the tool exists.
    pub fn set_tool_enabled(&self, tool_name: &str, enabled: bool) -> bool {
        if !self.tool_to_source.contains_key(tool_name) {
            return false;
        }
        self.enabled_overrides.insert(tool_name.to_owned(), enabled);
        debug!(tool = %tool_name, enabled, "Tool enabled state changed");
        true
    }

    /// Enable a tool. Returns `true` if the tool exists.
    pub fn enable_tool(&self, tool_name: &str) -> bool {
        self.set_tool_enabled(tool_name, true)
    }

    /// Disable a tool. Returns `true` if the tool exists.
    pub fn disable_tool(&self, tool_name: &str) -> bool {
        self.set_tool_enabled(tool_name, false)
    }
}

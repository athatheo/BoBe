use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use super::base::NativeTool;
use crate::error::AppError;
use crate::llm::types::{AiToolCall, ToolDefinition};
use crate::tools::{ToolCategory, ToolExecutionContext, ToolResult, ToolSource};

/// Aggregates all BoBe native tools as a single ToolSource.
pub struct NativeToolAdapter {
    tools: HashMap<String, Arc<dyn NativeTool>>,
    categories: Vec<ToolCategory>,
    disabled_tools: std::sync::RwLock<std::collections::HashSet<String>>,
}

impl NativeToolAdapter {
    pub fn new(tools: Vec<Arc<dyn NativeTool>>) -> Self {
        let mut categories = std::collections::HashSet::new();
        let mut map = HashMap::new();

        for tool in tools {
            categories.insert(tool.category());
            map.insert(tool.name().to_owned(), tool);
        }

        let cats: Vec<ToolCategory> = categories.into_iter().collect();

        Self {
            tools: map,
            categories: cats,
            disabled_tools: std::sync::RwLock::new(std::collections::HashSet::new()),
        }
    }

    pub fn enable_tool(&self, name: &str) -> bool {
        if !self.tools.contains_key(name) {
            return false;
        }
        let mut disabled = self.disabled_tools.write().unwrap();
        disabled.remove(name)
    }

    pub fn disable_tool(&self, name: &str) -> bool {
        if !self.tools.contains_key(name) {
            return false;
        }
        let mut disabled = self.disabled_tools.write().unwrap();
        disabled.insert(name.to_owned())
    }

    pub fn set_tool_enabled(&self, name: &str, enabled: bool) -> bool {
        if enabled {
            self.enable_tool(name)
        } else {
            self.disable_tool(name)
        }
    }

    pub fn is_tool_enabled(&self, name: &str) -> Option<bool> {
        if !self.tools.contains_key(name) {
            return None;
        }
        let disabled = self.disabled_tools.read().unwrap();
        Some(!disabled.contains(name))
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

#[async_trait]
impl ToolSource for NativeToolAdapter {
    fn name(&self) -> &str {
        "bobe"
    }

    fn categories(&self) -> &[ToolCategory] {
        &self.categories
    }

    async fn get_tools(&self, include_disabled: bool) -> Result<Vec<ToolDefinition>, AppError> {
        let disabled = self.disabled_tools.read().unwrap();
        let defs: Vec<ToolDefinition> = self
            .tools
            .values()
            .filter(|t| include_disabled || !disabled.contains(t.name()))
            .map(|t| ToolDefinition {
                name: t.name().to_owned(),
                description: t.description().to_owned(),
                parameters: t.parameters(),
            })
            .collect();
        Ok(defs)
    }

    async fn execute(
        &self,
        tool_call: &AiToolCall,
        context: Option<&ToolExecutionContext>,
    ) -> ToolResult {
        let tool = match self.tools.get(&tool_call.name) {
            Some(t) => t,
            None => {
                warn!(tool = %tool_call.name, "Native tool not found");
                return ToolResult::err(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    format!("Tool '{}' not found", tool_call.name),
                );
            }
        };

        // Check if disabled
        {
            let disabled = self.disabled_tools.read().unwrap();
            if disabled.contains(&tool_call.name) {
                return ToolResult::err(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    format!("Tool '{}' is disabled", tool_call.name),
                );
            }
        }

        debug!(tool = %tool_call.name, "Executing native tool");

        match tool.execute(tool_call.arguments.clone(), context).await {
            Ok(content) => ToolResult::ok(tool_call.id.clone(), tool_call.name.clone(), content),
            Err(e) => {
                warn!(tool = %tool_call.name, error = %e, "Native tool execution failed");
                ToolResult::err(tool_call.id.clone(), tool_call.name.clone(), e.to_string())
            }
        }
    }

    async fn health_check(&self) -> bool {
        true // Native tools are always in-process
    }
}

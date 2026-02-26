use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use super::base::NativeTool;
use crate::error::AppError;
use crate::llm::types::{AiToolCall, ToolDefinition};
use crate::tools::{ToolExecutionContext, ToolResult, ToolSource};

/// Aggregates all BoBe native tools as a single ToolSource.
pub struct NativeToolAdapter {
    tools: HashMap<String, Arc<dyn NativeTool>>,
    disabled_tools: std::sync::RwLock<std::collections::HashSet<String>>,
}

impl NativeToolAdapter {
    pub fn new(tools: Vec<Arc<dyn NativeTool>>) -> Self {
        let mut map = HashMap::new();

        for tool in tools {
            map.insert(tool.name().to_owned(), tool);
        }

        Self {
            tools: map,
            disabled_tools: std::sync::RwLock::new(std::collections::HashSet::new()),
        }
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

    async fn get_tools(&self, include_disabled: bool) -> Result<Vec<ToolDefinition>, AppError> {
        let disabled = read_lock_or_recover(&self.disabled_tools, "native_tool.disabled_tools");
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
            let disabled = read_lock_or_recover(&self.disabled_tools, "native_tool.disabled_tools");
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
}

fn read_lock_or_recover<'a, T>(
    lock: &'a std::sync::RwLock<T>,
    lock_name: &'static str,
) -> std::sync::RwLockReadGuard<'a, T> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!(lock = lock_name, "rwlock poisoned on read, recovering");
            poisoned.into_inner()
        }
    }
}

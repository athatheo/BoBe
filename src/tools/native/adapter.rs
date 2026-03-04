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
}

impl NativeToolAdapter {
    pub fn new(tools: Vec<Arc<dyn NativeTool>>) -> Self {
        let mut map = HashMap::new();

        for tool in tools {
            map.insert(tool.name().to_owned(), tool);
        }

        Self { tools: map }
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

    async fn get_tools(&self) -> Result<Vec<ToolDefinition>, AppError> {
        let defs: Vec<ToolDefinition> = self
            .tools
            .values()
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
        let Some(tool) = self.tools.get(&tool_call.name) else {
            warn!(tool = %tool_call.name, "Native tool not found");
            return ToolResult::err(
                tool_call.id.clone(),
                tool_call.name.clone(),
                format!("Tool '{}' not found", tool_call.name),
            );
        };

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

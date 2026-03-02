use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use super::base::NativeTool;
use crate::db::SoulRepository;
use crate::error::AppError;
use crate::tools::ToolExecutionContext;

pub struct GetSoulsTool {
    soul_repo: Arc<dyn SoulRepository>,
}

impl GetSoulsTool {
    pub fn new(soul_repo: Arc<dyn SoulRepository>) -> Self {
        Self { soul_repo }
    }
}

#[async_trait]
impl NativeTool for GetSoulsTool {
    fn name(&self) -> &str {
        "get_souls"
    }

    fn description(&self) -> &str {
        "Get personality documents that define BoBe's behavior and communication style."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let souls = self.soul_repo.find_enabled().await?;

        if souls.is_empty() {
            return Ok("No personality documents configured.".into());
        }

        let mut output = String::new();
        for soul in &souls {
            let default_marker = if soul.is_default { " (default)" } else { "" };
            let _ = write!(
                output,
                "## {}{}\n\n{}\n\n---\n\n",
                soul.name, default_marker, soul.content
            );
        }
        Ok(output)
    }
}

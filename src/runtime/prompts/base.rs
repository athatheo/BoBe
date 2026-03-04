//! Base prompt types.

use crate::llm::types::{ResponseFormat, ToolDefinition};

/// Default soul fallback (used when no soul provider is available).
pub const DEFAULT_SOUL: &str = crate::constants::DEFAULT_SOUL_FALLBACK;

/// LLM parameters for a prompt — declared once per prompt type.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PromptConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    pub tools: Vec<ToolDefinition>,
    pub response_format: Option<ResponseFormat>,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 1024,
            tools: Vec::new(),
            response_format: None,
        }
    }
}

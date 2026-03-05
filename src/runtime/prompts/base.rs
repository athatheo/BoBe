use crate::llm::types::{ResponseFormat, ToolDefinition};

pub(crate) const DEFAULT_SOUL: &str = crate::constants::DEFAULT_SOUL_FALLBACK;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct PromptConfig {
    pub(crate) temperature: f32,
    pub(crate) max_tokens: u32,
    pub(crate) tools: Vec<ToolDefinition>,
    pub(crate) response_format: Option<ResponseFormat>,
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

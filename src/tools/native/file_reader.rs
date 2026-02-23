use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::base::NativeTool;
use crate::error::AppError;
use crate::tools::{ToolCategory, ToolExecutionContext};

const MAX_FILE_SIZE: u64 = 1_048_576; // 1 MB
const MAX_LINES: usize = 500;

pub struct FileReaderTool;

impl Default for FileReaderTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReaderTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_path(path: &Path) -> Result<PathBuf, AppError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| AppError::Tool(format!("Cannot resolve path: {e}")))?;

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let tmp = PathBuf::from("/tmp");

        if !canonical.starts_with(&home) && !canonical.starts_with(&tmp) {
            return Err(AppError::Tool(format!(
                "Access denied: path must be under home directory or /tmp. Got: {}",
                canonical.display()
            )));
        }
        Ok(canonical)
    }
}

#[async_trait]
impl NativeTool for FileReaderTool {
    fn name(&self) -> &str {
        "file_reader"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Path must be absolute and within the user's home directory."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "max_lines": {
                    "type": "integer",
                    "description": "Maximum lines to read (default: 500, max: 500)",
                    "default": 500
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileSystem
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let path_str = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'path' is required".into()))?;

        let max_lines = arguments
            .get("max_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(MAX_LINES as u64)
            .min(MAX_LINES as u64) as usize;

        let path = Path::new(path_str);
        let canonical = Self::validate_path(path)?;

        let metadata = tokio::fs::metadata(&canonical)
            .await
            .map_err(|e| AppError::Tool(format!("Cannot read file metadata: {e}")))?;

        if !metadata.is_file() {
            return Err(AppError::Tool(format!(
                "{} is not a file",
                canonical.display()
            )));
        }

        if metadata.len() > MAX_FILE_SIZE {
            return Err(AppError::Tool(format!(
                "File too large ({} bytes). Maximum is {} bytes.",
                metadata.len(),
                MAX_FILE_SIZE
            )));
        }

        let content = tokio::fs::read_to_string(&canonical)
            .await
            .map_err(|e| AppError::Tool(format!("Cannot read file: {e}")))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let truncated = total_lines > max_lines;
        let display_lines = if truncated {
            &lines[..max_lines]
        } else {
            &lines[..]
        };

        let mut output = display_lines.join("\n");
        if truncated {
            output.push_str(&format!(
                "\n\n--- Truncated: showing {max_lines} of {total_lines} lines ---"
            ));
        }

        Ok(format!(
            "File: {}\nSize: {} bytes | Lines: {}{}\n\n{}",
            canonical.display(),
            metadata.len(),
            total_lines,
            if truncated {
                format!(" (showing {max_lines})")
            } else {
                String::new()
            },
            output,
        ))
    }
}

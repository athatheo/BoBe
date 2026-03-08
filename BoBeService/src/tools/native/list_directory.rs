use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use super::base::NativeTool;
use crate::error::AppError;
use crate::tools::ToolExecutionContext;

const MAX_ENTRIES: usize = 500;
const MAX_RECURSIVE_DEPTH: usize = 2;

pub(crate) struct ListDirectoryTool;

impl Default for ListDirectoryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ListDirectoryTool {
    pub(crate) fn new() -> Self {
        Self
    }

    fn format_size(size: u64) -> String {
        if size < 1024 {
            format!("{size} B")
        } else if size < 1_048_576 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else if size < 1_073_741_824 {
            format!("{:.1} MB", size as f64 / 1_048_576.0)
        } else {
            format!("{:.1} GB", size as f64 / 1_073_741_824.0)
        }
    }
}

#[async_trait]
impl NativeTool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List directory contents with optional filtering and recursive traversal."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to directory"
                },
                "pattern": {
                    "type": "string",
                    "description": "Glob filter pattern (default: *)",
                    "default": "*"
                },
                "show_hidden": {
                    "type": "boolean",
                    "description": "Include hidden (dot) files (default: false)",
                    "default": false
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Recurse into subdirectories up to 2 levels (default: false)",
                    "default": false
                }
            },
            "required": ["path"]
        })
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

        let show_hidden = arguments
            .get("show_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let recursive = arguments
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let canonical = super::path_validation::validate_path(Path::new(path_str))?;

        if !canonical.is_dir() {
            return Err(AppError::Tool(format!(
                "{} is not a directory",
                canonical.display()
            )));
        }

        let max_depth = if recursive { MAX_RECURSIVE_DEPTH } else { 0 };
        let mut entries = Vec::new();

        collect_entries(
            &canonical,
            &canonical,
            max_depth,
            0,
            show_hidden,
            &mut entries,
        )
        .await?;

        // Sort: directories first, then alphabetical
        entries.sort_by(|a, b| {
            let a_dir = a.starts_with("📁");
            let b_dir = b.starts_with("📁");
            b_dir.cmp(&a_dir).then_with(|| a.cmp(b))
        });

        let total = entries.len();
        let truncated = total > MAX_ENTRIES;
        let display = if truncated {
            &entries[..MAX_ENTRIES]
        } else {
            &entries[..]
        };

        let mut output = format!("Directory: {}\n{} entries", canonical.display(), total);
        if truncated {
            let _ = write!(output, " (showing {MAX_ENTRIES})");
        }
        output.push_str("\n\n");
        output.push_str(&display.join("\n"));

        Ok(output)
    }
}

async fn collect_entries(
    base: &Path,
    dir: &Path,
    max_depth: usize,
    current_depth: usize,
    show_hidden: bool,
    entries: &mut Vec<String>,
) -> Result<(), AppError> {
    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| AppError::Tool(format!("Cannot read directory: {e}")))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| AppError::Tool(format!("Error reading entry: {e}")))?
    {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if !show_hidden && name.starts_with('.') {
            continue;
        }

        if entries.len() >= MAX_ENTRIES {
            return Ok(());
        }

        let Ok(metadata) = entry.metadata().await else {
            continue;
        };

        let relative = entry
            .path()
            .strip_prefix(base)
            .unwrap_or(&entry.path())
            .to_string_lossy()
            .to_string();

        if metadata.is_dir() {
            entries.push(format!("📁 {relative}/"));
            if current_depth < max_depth {
                Box::pin(collect_entries(
                    base,
                    &entry.path(),
                    max_depth,
                    current_depth + 1,
                    show_hidden,
                    entries,
                ))
                .await?;
            }
        } else {
            let size = ListDirectoryTool::format_size(metadata.len());
            entries.push(format!("📄 {relative}  ({size})"));
        }
    }
    Ok(())
}

use async_trait::async_trait;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::base::NativeTool;
use crate::error::AppError;
use crate::tools::ToolExecutionContext;

const MAX_RESULTS: usize = 100;

pub struct SearchFilesTool;

impl Default for SearchFilesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchFilesTool {
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
            return Err(AppError::Tool(
                "Access denied: path must be under home directory or /tmp".into(),
            ));
        }

        // Re-validate after canonicalize to prevent symlink-swap TOCTOU attacks
        let re_canon = canonical
            .canonicalize()
            .map_err(|e| AppError::Tool(format!("Path changed during validation: {e}")))?;
        if !re_canon.starts_with(&home) && !re_canon.starts_with(&tmp) {
            return Err(AppError::Tool(
                "Access denied: path escaped allowed directories after re-validation".into(),
            ));
        }

        Ok(re_canon)
    }

    fn validate_regex(pattern: &str) -> Result<(), AppError> {
        // Simple ReDoS protection: limit quantifier nesting
        let quantifier_count = pattern.matches('*').count()
            + pattern.matches('+').count()
            + pattern.matches('?').count();
        if quantifier_count > 5 {
            return Err(AppError::Validation(
                "Pattern too complex: max 5 quantifiers allowed".into(),
            ));
        }
        Regex::new(pattern).map_err(|e| AppError::Validation(format!("Invalid regex: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl NativeTool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }

    fn description(&self) -> &str {
        "Search file contents using text or regex patterns. Returns matching lines with file paths and line numbers."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to search in (absolute path)"
                },
                "query": {
                    "type": "string",
                    "description": "Text or regex pattern to search for"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Glob pattern for file names (e.g., '*.rs')"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case sensitive search (default: false)",
                    "default": false
                },
                "regex": {
                    "type": "boolean",
                    "description": "Treat query as regex (default: false)",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results (default: 100)",
                    "default": 100
                }
            },
            "required": ["path", "query"]
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

        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'query' is required".into()))?;

        let case_sensitive = arguments
            .get("case_sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let use_regex = arguments
            .get("regex")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let max_results = arguments
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(MAX_RESULTS as u64)
            .min(MAX_RESULTS as u64) as usize;

        let file_pattern = arguments
            .get("file_pattern")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let canonical = Self::validate_path(Path::new(path_str))?;

        if !canonical.is_dir() {
            return Err(AppError::Tool(format!(
                "{} is not a directory",
                canonical.display()
            )));
        }

        // Build pattern
        let pattern = if use_regex {
            Self::validate_regex(query)?;
            if case_sensitive {
                query.to_owned()
            } else {
                format!("(?i){query}")
            }
        } else {
            let escaped = regex::escape(query);
            if case_sensitive {
                escaped
            } else {
                format!("(?i){escaped}")
            }
        };

        let re = Regex::new(&pattern).map_err(|e| AppError::Tool(format!("Pattern error: {e}")))?;

        // Search via tokio::task::spawn_blocking for heavy IO
        let re_clone = re.clone();
        let file_pattern_clone = file_pattern.clone();
        let results = tokio::task::spawn_blocking(move || {
            search_directory(
                &canonical,
                &re_clone,
                file_pattern_clone.as_deref(),
                max_results,
            )
        })
        .await
        .map_err(|e| AppError::Tool(format!("Search task failed: {e}")))?;

        if results.is_empty() {
            return Ok("No matches found.".into());
        }

        let mut output = format!("Found {} matches:\n\n", results.len());
        for (file, line_num, line) in &results {
            output.push_str(&format!("{}:{}: {}\n", file, line_num, line.trim()));
        }
        Ok(output)
    }
}

fn search_directory(
    dir: &Path,
    re: &Regex,
    file_pattern: Option<&str>,
    max_results: usize,
) -> Vec<(String, usize, String)> {
    let mut results = Vec::new();
    let walker = walkdir::WalkDir::new(dir).max_depth(10).into_iter();

    for entry in walker.filter_map(|e| e.ok()) {
        if results.len() >= max_results {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if let Some(pattern) = file_pattern
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
            && !glob_match(pattern, name)
        {
            continue;
        }

        // Skip binary files
        if let Ok(content) = std::fs::read_to_string(path) {
            let display_path = path.to_string_lossy().to_string();
            for (line_num, line) in content.lines().enumerate() {
                if results.len() >= max_results {
                    break;
                }
                if re.is_match(line) {
                    results.push((display_path.clone(), line_num + 1, line.to_owned()));
                }
            }
        }
    }
    results
}

/// Simple glob matching for file patterns (supports * and ?).
fn glob_match(pattern: &str, name: &str) -> bool {
    let re_pattern = format!(
        "^{}$",
        regex::escape(pattern)
            .replace(r"\*", ".*")
            .replace(r"\?", ".")
    );
    Regex::new(&re_pattern)
        .map(|re| re.is_match(name))
        .unwrap_or(false)
}

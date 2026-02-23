use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

const DEFAULT_SEARCH_DIRS: &[&str] = &[
    "Repos",
    "Projects",
    "code",
    "src",
    "dev",
    "workspace",
    "github",
    "git",
];
const MAX_DEPTH: usize = 3;
const MAX_REPOS: usize = 50;

pub struct DiscoverGitReposTool;

impl Default for DiscoverGitReposTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoverGitReposTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NativeTool for DiscoverGitReposTool {
    fn name(&self) -> &str {
        "discover_git_repos"
    }

    fn description(&self) -> &str {
        "Discover git repositories by searching common project directories."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "search_dir": {
                    "type": "string",
                    "description": "Specific directory to search (defaults to common locations like ~/Repos, ~/Projects)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let home =
            dirs::home_dir().ok_or_else(|| AppError::Tool("Cannot find home directory".into()))?;

        let search_dirs: Vec<PathBuf> =
            if let Some(dir) = arguments.get("search_dir").and_then(|v| v.as_str()) {
                vec![PathBuf::from(dir)]
            } else {
                DEFAULT_SEARCH_DIRS
                    .iter()
                    .map(|d| home.join(d))
                    .filter(|p| p.exists())
                    .collect()
            };

        if search_dirs.is_empty() {
            return Ok("No standard project directories found.".into());
        }

        let mut repos = Vec::new();
        for dir in &search_dirs {
            find_git_repos(dir, 0, &mut repos);
            if repos.len() >= MAX_REPOS {
                break;
            }
        }
        repos.truncate(MAX_REPOS);

        if repos.is_empty() {
            return Ok("No git repositories found.".into());
        }

        // Get metadata for each repo
        let mut output = format!("Found {} git repositories:\n\n", repos.len());
        for repo_path in &repos {
            let info = get_repo_info(repo_path).await;
            output.push_str(&format!(
                "📁 {}\n   Branch: {} | Remote: {}\n   Last commit: {}\n\n",
                repo_path.display(),
                info.branch,
                info.remote,
                info.last_commit,
            ));
        }
        Ok(output)
    }
}

fn find_git_repos(dir: &Path, depth: usize, repos: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || repos.len() >= MAX_REPOS {
        return;
    }

    let git_dir = dir.join(".git");
    if git_dir.exists() {
        repos.push(dir.to_path_buf());
        return; // Don't recurse into git repos
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.starts_with('.') && name_str != "node_modules" && name_str != "target"
                {
                    find_git_repos(&entry.path(), depth + 1, repos);
                }
            }
        }
    }
}

struct RepoInfo {
    branch: String,
    remote: String,
    last_commit: String,
}

async fn get_repo_info(path: &Path) -> RepoInfo {
    let path = path.to_path_buf();

    let branch = run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    let remote = run_git(&path, &["remote", "get-url", "origin"]).await;
    let last_commit = run_git(&path, &["log", "-1", "--format=%h %s (%cr)"]).await;

    RepoInfo {
        branch: branch.unwrap_or_else(|| "unknown".into()),
        remote: remote.unwrap_or_else(|| "none".into()),
        last_commit: last_commit.unwrap_or_else(|| "no commits".into()),
    }
}

async fn run_git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}

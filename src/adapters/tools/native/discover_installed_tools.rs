use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;

use super::base::NativeTool;
use crate::error::AppError;
use crate::ports::tools::{ToolCategory, ToolExecutionContext};

/// Known dev tools to check with --version or similar flags.
const DEV_TOOLS: &[(&str, &[&str])] = &[
    ("git", &["--version"]),
    ("python3", &["--version"]),
    ("node", &["--version"]),
    ("npm", &["--version"]),
    ("rustc", &["--version"]),
    ("cargo", &["--version"]),
    ("go", &["version"]),
    ("java", &["--version"]),
    ("docker", &["--version"]),
    ("docker-compose", &["--version"]),
    ("kubectl", &["version", "--client", "--short"]),
    ("terraform", &["--version"]),
    ("aws", &["--version"]),
    ("az", &["--version"]),
    ("gcloud", &["--version"]),
    ("make", &["--version"]),
    ("cmake", &["--version"]),
    ("gcc", &["--version"]),
    ("clang", &["--version"]),
    ("ruby", &["--version"]),
    ("swift", &["--version"]),
    ("dotnet", &["--version"]),
];

pub struct DiscoverInstalledToolsTool;

impl Default for DiscoverInstalledToolsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoverInstalledToolsTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NativeTool for DiscoverInstalledToolsTool {
    fn name(&self) -> &str {
        "discover_installed_tools"
    }

    fn description(&self) -> &str {
        "Discover installed development tools and package managers on this system."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    async fn execute(
        &self,
        _arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let mut output = String::from("## Installed Development Tools\n\n");

        // Check package manager (macOS: Homebrew)
        if let Some(brew_info) = check_tool("brew", &["--version"]).await {
            output.push_str(&format!("### Package Manager\n• Homebrew: {brew_info}\n\n"));

            // List installed packages
            if let Some(packages) = run_command("brew", &["list", "--formula", "-1"]).await {
                let count = packages.lines().count();
                let preview: String = packages.lines().take(20).collect::<Vec<_>>().join(", ");
                output.push_str(&format!(
                    "  Installed formulae: {count}\n  Sample: {preview}\n\n"
                ));
            }
        }

        output.push_str("### Development Tools\n\n");
        for (tool, args) in DEV_TOOLS {
            if let Some(version) = check_tool(tool, args).await {
                let first_line = version.lines().next().unwrap_or(&version);
                output.push_str(&format!("• {tool}: {first_line}\n"));
            }
        }

        Ok(output)
    }
}

async fn check_tool(cmd: &str, args: &[&str]) -> Option<String> {
    let result = tokio::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .ok()?;

    if result.status.success() {
        let out = String::from_utf8_lossy(&result.stdout).trim().to_owned();
        if out.is_empty() {
            Some(String::from_utf8_lossy(&result.stderr).trim().to_owned())
        } else {
            Some(out)
        }
    } else {
        None
    }
}

async fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    let result = tokio::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    if result.status.success() {
        Some(String::from_utf8_lossy(&result.stdout).trim().to_owned())
    } else {
        None
    }
}

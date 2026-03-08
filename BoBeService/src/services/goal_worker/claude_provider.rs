//! ClaudeAgentProvider — implements `GoalExecutorProvider` via the Claude CLI.
//!
//! Two-phase approach per goal:
//! - Phase A (Planning): Call Anthropic API to generate a structured plan.
//! - Phase B (Execution): Shell out to the `claude` CLI for autonomous execution.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::Config;
use crate::error::AppError;
use crate::models::goal::Goal;
use crate::models::goal_plan::{GoalPlan, GoalPlanStep};
use crate::models::ids::GoalId;
use crate::runtime::prompts::goal_worker::{GoalExecutionPrompt, GoalPlanningPrompt};
use crate::util::slugify::slugify;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use secrecy::ExposeSecret;
use tracing::{info, warn};

use super::{GoalExecutionResult, GoalExecutorProvider, PlanStep};

pub(crate) struct ClaudeAgentProvider {
    config: Arc<ArcSwap<Config>>,
    http_client: reqwest::Client,
}

impl ClaudeAgentProvider {
    pub(crate) fn new(config: Arc<ArcSwap<Config>>, http_client: reqwest::Client) -> Self {
        Self {
            config,
            http_client,
        }
    }

    fn cfg(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}

#[async_trait]
impl GoalExecutorProvider for ClaudeAgentProvider {
    fn create_work_dir(&self, goal_id: GoalId, goal_title: &str) -> PathBuf {
        let cfg = self.cfg();
        let base = cfg.resolved_projects_dir();

        let slug = slugify(goal_title, 50);
        let short_id = &goal_id.to_string()[..8];
        let dir_name = if slug.is_empty() {
            short_id.to_string()
        } else {
            format!("{slug}-{short_id}")
        };

        let work_dir = base.join(dir_name);
        if let Err(e) = std::fs::create_dir_all(&work_dir) {
            warn!(
                error = %e,
                goal_id = %goal_id,
                "claude_provider.work_dir_creation_failed, falling back to UUID"
            );
            let fallback = base.join(goal_id.to_string());
            let _ignored = std::fs::create_dir_all(&fallback);
            return fallback;
        }
        work_dir
    }

    async fn generate_plan(
        &self,
        goal: &Goal,
        context: &str,
        max_steps: Option<u32>,
    ) -> Result<Vec<PlanStep>, AppError> {
        let cfg = self.cfg();
        let effective_max_steps = max_steps.unwrap_or(cfg.goal_worker.plan_max_steps);
        let api_key = cfg.llm.anthropic_api_key.expose_secret().to_owned();
        let model = cfg.goal_worker.claude_model.clone();
        let locale = cfg.effective_locale();
        drop(cfg);

        if api_key.is_empty() {
            return Err(AppError::Config(
                "BOBE_ANTHROPIC_API_KEY not set".to_string(),
            ));
        }

        let (system_msg, user_msg) = GoalPlanningPrompt::messages(
            &goal.content,
            context,
            effective_max_steps,
            Some(&locale),
        );

        let body = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "system": system_msg,
            "messages": [{"role": "user", "content": user_msg}],
        });

        let resp = self
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Llm(format!("Anthropic API request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llm(format!(
                "Anthropic API error {status}: {text}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Llm(format!("Failed to parse Anthropic response: {e}")))?;

        let plan_text = extract_text_from_response(&data);

        let steps = parse_plan(&plan_text, effective_max_steps);

        info!(
            goal_id = %goal.id,
            step_count = steps.len(),
            goal_preview = &goal.content[..goal.content.len().min(60)],
            "claude_provider.plan_generated"
        );

        Ok(steps)
    }

    async fn execute_goal(
        &self,
        goal: &Goal,
        _plan: &GoalPlan,
        steps: &[GoalPlanStep],
        work_dir: &Path,
    ) -> Result<GoalExecutionResult, AppError> {
        let cfg = self.cfg();
        let model = cfg.goal_worker.claude_model.clone();
        let api_key = cfg.llm.anthropic_api_key.expose_secret().to_owned();
        let locale = cfg.effective_locale();
        drop(cfg);

        let step_list: String = steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s.content))
            .collect::<Vec<_>>()
            .join("\n");

        let (system_msg, user_msg) =
            GoalExecutionPrompt::messages(&goal.content, &step_list, work_dir, Some(&locale));

        // Try the Claude CLI first (preferred — it has tool use built in)
        match execute_via_cli(&model, &api_key, &system_msg, &user_msg, work_dir).await {
            Ok(result) => {
                info!(
                    goal_id = %goal.id,
                    success = result.success,
                    work_dir = %work_dir.display(),
                    output_len = result.output.len(),
                    "claude_provider.execution_complete"
                );
                return Ok(result);
            }
            Err(cli_err) => {
                warn!(
                    error = %cli_err,
                    "claude_provider.cli_unavailable, falling back to API"
                );
            }
        }

        // Fallback: call Anthropic Messages API directly (no tool use)
        execute_via_api(&self.http_client, &model, &api_key, &system_msg, &user_msg).await
    }
}

// ─── CLI execution ──────────────────────────────────────────────────────────

async fn execute_via_cli(
    model: &str,
    api_key: &str,
    system_prompt: &str,
    user_message: &str,
    work_dir: &Path,
) -> Result<GoalExecutionResult, AppError> {
    let claude_bin =
        which::which("claude").map_err(|e| AppError::Tool(format!("claude CLI not found: {e}")))?;

    let prompt = format!("System: {system_prompt}\n\n{user_message}");

    let mut cmd = tokio::process::Command::new(claude_bin);
    cmd.arg("--model")
        .arg(model)
        .arg("--print")
        .arg("--output-format")
        .arg("json")
        .current_dir(work_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if !api_key.is_empty() {
        cmd.env("ANTHROPIC_API_KEY", api_key);
    }
    cmd.env("CLAUDECODE", ""); // bypass nested-session guard

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Tool(format!("Failed to spawn claude CLI: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        drop(stdin.write_all(prompt.as_bytes()).await);
        drop(stdin);
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Tool(format!("claude CLI execution failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        let result_text = parse_cli_output(&stdout).unwrap_or(stdout);
        Ok(GoalExecutionResult {
            success: true,
            output: result_text,
            error: None,
        })
    } else {
        Ok(GoalExecutionResult {
            success: false,
            output: stdout,
            error: Some(if stderr.is_empty() {
                format!("claude CLI exited with status {}", output.status)
            } else {
                stderr
            }),
        })
    }
}

fn parse_cli_output(stdout: &str) -> Option<String> {
    let data: serde_json::Value = serde_json::from_str(stdout).ok()?;
    if let Some(result) = data.get("result").and_then(|v| v.as_str()) {
        return Some(result.to_string());
    }
    if let Some(content) = data.get("content").and_then(|v| v.as_array()) {
        let text: String = content
            .iter()
            .filter_map(|block| {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    block.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

// ─── API fallback ───────────────────────────────────────────────────────────

async fn execute_via_api(
    client: &reqwest::Client,
    model: &str,
    api_key: &str,
    system_msg: &str,
    user_msg: &str,
) -> Result<GoalExecutionResult, AppError> {
    if api_key.is_empty() {
        return Err(AppError::Config(
            "BOBE_ANTHROPIC_API_KEY not set".to_string(),
        ));
    }

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 8192,
        "system": system_msg,
        "messages": [{"role": "user", "content": user_msg}],
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Llm(format!("Anthropic API request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Ok(GoalExecutionResult {
            success: false,
            output: String::new(),
            error: Some(format!("Anthropic API error {status}: {text}")),
        });
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Llm(format!("Failed to parse Anthropic response: {e}")))?;

    let output = extract_text_from_response(&data);

    Ok(GoalExecutionResult {
        success: true,
        output,
        error: None,
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn extract_text_from_response(data: &serde_json::Value) -> String {
    let Some(content) = data.get("content").and_then(|v| v.as_array()) else {
        return String::new();
    };
    content
        .iter()
        .filter_map(|block| {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                block.get("text").and_then(|t| t.as_str()).map(String::from)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parses JSON `{"steps": [...]}` or falls back to numbered-list extraction.
fn parse_plan(plan_text: &str, max_steps: u32) -> Vec<PlanStep> {
    let text = plan_text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let json_text = strip_code_fences(text);

    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_text)
        && let Some(raw_steps) = data.get("steps").and_then(|v| v.as_array())
    {
        let mut steps = Vec::new();
        for (i, step) in raw_steps.iter().enumerate() {
            if i >= max_steps as usize {
                break;
            }
            let content = if let Some(obj) = step.as_object() {
                obj.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string()
            } else if let Some(s) = step.as_str() {
                s.to_string()
            } else {
                continue;
            };
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                steps.push(PlanStep {
                    content: trimmed.to_string(),
                    order: i as i32,
                });
            }
        }
        return steps;
    }

    // Fallback: parse as numbered list
    warn!("claude_provider.plan_parse_fallback");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .take(max_steps as usize)
        .enumerate()
        .filter_map(|(i, line)| {
            let content = line.trim().trim_start_matches(|c: char| {
                c.is_ascii_digit() || c == '.' || c == '-' || c == ')' || c == ' '
            });
            if content.is_empty() {
                None
            } else {
                Some(PlanStep {
                    content: content.to_string(),
                    order: i as i32,
                })
            }
        })
        .collect()
}

fn strip_code_fences(text: &str) -> String {
    if let Some(start_idx) = text.find("```json") {
        let after_fence = &text[start_idx + 7..];
        if let Some(end_idx) = after_fence.find("```") {
            return after_fence[..end_idx].trim().to_string();
        }
    }
    if let Some(start_idx) = text.find("```") {
        let after_fence = &text[start_idx + 3..];
        if let Some(end_idx) = after_fence.find("```") {
            return after_fence[..end_idx].trim().to_string();
        }
    }
    text.to_string()
}

use std::sync::Arc;

use github_copilot_sdk::tool::define_tool;
use github_copilot_sdk::types::{Tool, ToolResult, ToolResultExpanded};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::ids::GoalId;
use crate::models::types::GoalStatus;
use crate::services::goals::goal_md::GoalDoc;
use crate::services::goals::goals_service::{GoalPatch, GoalsService};

use super::memory_file::MemoryFile;

pub(crate) const MEMORY_APPEND: &str = "bobe_memory_append";
pub(crate) const GOAL_LIST: &str = "bobe_goal_list";
pub(crate) const GOAL_CREATE: &str = "bobe_goal_create";
pub(crate) const GOAL_UPDATE: &str = "bobe_goal_update";

pub(crate) const CHAT_DOMAIN_TOOL_NAMES: &[&str] =
    &[MEMORY_APPEND, GOAL_LIST, GOAL_CREATE, GOAL_UPDATE];

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum MemorySection {
    Profile,
    ActiveGoals,
    LongTerm,
    Recent,
}

impl MemorySection {
    const fn heading(self) -> &'static str {
        match self {
            Self::Profile => "Profile",
            Self::ActiveGoals => "Active Goals",
            Self::LongTerm => "Long-term",
            Self::Recent => "Recent",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MemoryAppendParams {
    /// Memory section to update.
    section: MemorySection,
    /// One concise durable fact, preference, event, or observation.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GoalListParams {
    /// Include completed and archived goals. Defaults to active/paused goals only.
    #[serde(default)]
    include_inactive: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GoalCreateParams {
    /// Short human-readable goal title.
    title: String,
    /// Current concrete framing of the goal.
    #[serde(default)]
    summary: String,
    /// Why this goal matters to the user.
    #[serde(default)]
    why_it_matters: String,
    /// Urgency from 0 (low) through 5 (highest). Defaults to 2.
    priority: Option<u8>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum GoalStatusParam {
    Active,
    Paused,
    Completed,
    Archived,
}

impl From<GoalStatusParam> for GoalStatus {
    fn from(status: GoalStatusParam) -> Self {
        match status {
            GoalStatusParam::Active => Self::Active,
            GoalStatusParam::Paused => Self::Paused,
            GoalStatusParam::Completed => Self::Completed,
            GoalStatusParam::Archived => Self::Archived,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GoalUpdateParams {
    /// Goal UUID returned by bobe_goal_list or bobe_goal_create.
    id: String,
    title: Option<String>,
    status: Option<GoalStatusParam>,
    priority: Option<u8>,
    summary: Option<String>,
    why_it_matters: Option<String>,
    how_working_on_it: Option<String>,
    patterns_observed: Option<String>,
    attitude_feelings: Option<String>,
    open_questions: Option<String>,
    notes: Option<String>,
}

pub(crate) fn chat_domain_tools(
    memory_file: Arc<MemoryFile>,
    goals_service: Arc<GoalsService>,
) -> Vec<Tool> {
    // These exact tools are the permission boundary: they expose no paths or
    // commands and revalidate through MemoryFile/GoalsService. The pinned CLI
    // omits `kind` and tool name on their permission request, so the generic
    // handler cannot distinguish them; skip that prompt and authorize here.
    let memory_tool = {
        let memory_file = Arc::clone(&memory_file);
        define_tool(
            MEMORY_APPEND,
            "Append one durable fact to BoBe's structured memory. Never use for transient chat.",
            move |_invocation, params: MemoryAppendParams| {
                let memory_file = Arc::clone(&memory_file);
                async move {
                    Ok::<_, github_copilot_sdk::Error>(
                        append_memory(memory_file.as_ref(), params).await,
                    )
                }
            },
        )
        .with_skip_permission(true)
    };

    let list_tool = {
        let goals_service = Arc::clone(&goals_service);
        define_tool(
            GOAL_LIST,
            "List the user's BoBe goals before discussing or changing them.",
            move |_invocation, params: GoalListParams| {
                let goals_service = Arc::clone(&goals_service);
                async move {
                    Ok::<_, github_copilot_sdk::Error>(
                        list_goals(goals_service.as_ref(), params).await,
                    )
                }
            },
        )
        .with_skip_permission(true)
    };

    let create_tool = {
        let goals_service = Arc::clone(&goals_service);
        define_tool(
            GOAL_CREATE,
            "Create a goal only after the user explicitly agrees to track it.",
            move |_invocation, params: GoalCreateParams| {
                let goals_service = Arc::clone(&goals_service);
                async move {
                    Ok::<_, github_copilot_sdk::Error>(
                        create_goal(goals_service.as_ref(), params).await,
                    )
                }
            },
        )
        .with_skip_permission(true)
    };

    let update_tool = {
        let goals_service = Arc::clone(&goals_service);
        define_tool(
            GOAL_UPDATE,
            "Update an existing goal's living-document fields or lifecycle status.",
            move |_invocation, params: GoalUpdateParams| {
                let goals_service = Arc::clone(&goals_service);
                async move {
                    Ok::<_, github_copilot_sdk::Error>(
                        update_goal(goals_service.as_ref(), params).await,
                    )
                }
            },
        )
        .with_skip_permission(true)
    };

    vec![memory_tool, list_tool, create_tool, update_tool]
}

async fn append_memory(memory_file: &MemoryFile, params: MemoryAppendParams) -> ToolResult {
    if params.text.trim().is_empty() {
        return failure("memory text must not be empty");
    }
    if params.text.contains(['\r', '\n']) {
        return failure("memory text must be a single line");
    }
    match memory_file
        .append_under(params.section.heading(), &params.text)
        .await
    {
        Ok(()) => success(json!({
            "ok": true,
            "section": params.section.heading(),
            "revision": memory_file.revision(),
        })),
        Err(error) => failure(error),
    }
}

async fn list_goals(goals_service: &GoalsService, params: GoalListParams) -> ToolResult {
    match goals_service.list_all().await {
        Ok(goals) => {
            let goals = goals
                .into_iter()
                .filter(|goal| {
                    params.include_inactive
                        || matches!(goal.status, GoalStatus::Active | GoalStatus::Paused)
                })
                .map(|goal| {
                    json!({
                        "id": goal.id,
                        "title": goal.title,
                        "status": goal.status,
                        "priority": goal.priority,
                        "summary": goal.summary,
                        "why_it_matters": goal.why_it_matters,
                        "how_working_on_it": goal.how_working_on_it,
                        "patterns_observed": goal.patterns_observed,
                        "attitude_feelings": goal.attitude_feelings,
                        "open_questions": goal.open_questions,
                        "notes": goal.notes,
                    })
                })
                .collect::<Vec<_>>();
            success(json!({ "ok": true, "count": goals.len(), "goals": goals }))
        }
        Err(error) => failure(error),
    }
}

async fn create_goal(goals_service: &GoalsService, params: GoalCreateParams) -> ToolResult {
    let mut goal = GoalDoc::new(params.title, params.summary);
    goal.why_it_matters = params.why_it_matters;
    goal.priority = params.priority.unwrap_or(2);

    match goals_service.create(goal).await {
        Ok(goal) => success(goal_result(&goal)),
        Err(error) => failure(error),
    }
}

async fn update_goal(goals_service: &GoalsService, params: GoalUpdateParams) -> ToolResult {
    let id = match params.id.parse::<GoalId>() {
        Ok(id) => id,
        Err(error) => return failure(format!("invalid goal id: {error}")),
    };
    match goals_service
        .update(
            id,
            GoalPatch {
                title: params.title,
                status: params.status.map(GoalStatus::from),
                priority: params.priority,
                summary: params.summary,
                why_it_matters: params.why_it_matters,
                how_working_on_it: params.how_working_on_it,
                patterns_observed: params.patterns_observed,
                attitude_feelings: params.attitude_feelings,
                open_questions: params.open_questions,
                notes: params.notes,
            },
        )
        .await
    {
        Ok(Some(goal)) => success(goal_result(&goal)),
        Ok(None) => failure(format!("goal {id} was not found")),
        Err(error) => failure(error),
    }
}

fn goal_result(goal: &GoalDoc) -> serde_json::Value {
    json!({
        "ok": true,
        "id": goal.id,
        "title": goal.title,
        "status": goal.status,
        "priority": goal.priority,
        "summary": goal.summary,
        "why_it_matters": goal.why_it_matters,
        "how_working_on_it": goal.how_working_on_it,
        "patterns_observed": goal.patterns_observed,
        "attitude_feelings": goal.attitude_feelings,
        "open_questions": goal.open_questions,
        "notes": goal.notes,
        "updated_at": goal.updated_at,
    })
}

fn success(value: impl Serialize) -> ToolResult {
    match serde_json::to_string(&value) {
        Ok(value) => ToolResult::Text(value),
        Err(error) => failure(error),
    }
}

fn failure(error: impl std::fmt::Display) -> ToolResult {
    ToolResult::Expanded(ToolResultExpanded::new(
        json!({ "ok": false, "error": error.to_string() }).to_string(),
        "failure",
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on fixture failures")]
mod tests {
    use super::*;
    use crate::services::goals::file_store::GoalFileStore;

    fn temp_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("bobe-domain-tools-{}", uuid::Uuid::new_v4()))
    }

    fn text(result: ToolResult) -> String {
        match result {
            ToolResult::Text(text) => text,
            ToolResult::Expanded(expanded) => expanded.text_result_for_llm,
            _ => panic!("unexpected future ToolResult variant"),
        }
    }

    #[tokio::test]
    async fn memory_append_uses_bounded_section_names() {
        let root = temp_root();
        let memory = MemoryFile::new(root.join("memory.md"));

        let result = append_memory(
            memory.as_ref(),
            MemoryAppendParams {
                section: MemorySection::LongTerm,
                text: "User prefers concise answers".into(),
            },
        )
        .await;

        assert!(text(result).contains(r#""ok":true"#));
        assert!(
            memory
                .read()
                .await
                .unwrap()
                .contains("User prefers concise answers")
        );
        drop(tokio::fs::remove_dir_all(root).await);
    }

    #[tokio::test]
    async fn goal_tools_create_list_and_update_through_service() {
        let root = temp_root();
        let service = GoalsService::new(GoalFileStore::new(root.join("goals")));

        let created = create_goal(
            &service,
            GoalCreateParams {
                title: "Learn Greek".into(),
                summary: "Practice every week".into(),
                why_it_matters: "Family".into(),
                priority: Some(4),
            },
        )
        .await;
        let created: serde_json::Value = serde_json::from_str(&text(created)).unwrap();
        let id = created["id"].as_str().unwrap().to_owned();

        let updated = update_goal(
            &service,
            GoalUpdateParams {
                id,
                title: None,
                status: Some(GoalStatusParam::Paused),
                priority: None,
                summary: None,
                why_it_matters: None,
                how_working_on_it: Some("- Weekly lessons".into()),
                patterns_observed: Some("Momentum improves with a fixed schedule".into()),
                attitude_feelings: Some("Motivated".into()),
                open_questions: Some("Which dialect matters most?".into()),
                notes: Some("Paused for travel".into()),
            },
        )
        .await;
        let updated: serde_json::Value = serde_json::from_str(&text(updated)).unwrap();
        assert_eq!(updated["status"], "paused");
        assert_eq!(updated["how_working_on_it"], "- Weekly lessons");
        assert_eq!(
            updated["patterns_observed"],
            "Momentum improves with a fixed schedule"
        );
        assert_eq!(updated["attitude_feelings"], "Motivated");
        assert_eq!(updated["open_questions"], "Which dialect matters most?");

        let listed = list_goals(
            &service,
            GoalListParams {
                include_inactive: false,
            },
        )
        .await;
        let listed: serde_json::Value = serde_json::from_str(&text(listed)).unwrap();
        assert_eq!(listed["count"], 1);
        drop(tokio::fs::remove_dir_all(root).await);
    }

    #[tokio::test]
    async fn goal_create_rejects_invalid_priority() {
        let root = temp_root();
        let service = GoalsService::new(GoalFileStore::new(root.join("goals")));
        let result = create_goal(
            &service,
            GoalCreateParams {
                title: "Too urgent".into(),
                summary: String::new(),
                why_it_matters: String::new(),
                priority: Some(9),
            },
        )
        .await;

        assert!(text(result).contains("priority must be 0-5"));
        assert!(service.list_all().await.unwrap().is_empty());
        drop(tokio::fs::remove_dir_all(root).await);
    }

    #[tokio::test]
    async fn tools_reject_markdown_structure_injection() {
        let root = temp_root();
        let memory = MemoryFile::new(root.join("memory.md"));
        let memory_result = append_memory(
            memory.as_ref(),
            MemoryAppendParams {
                section: MemorySection::Recent,
                text: "Fact\n## Active Goals\nInjected".into(),
            },
        )
        .await;
        assert!(text(memory_result).contains("single line"));

        let service = GoalsService::new(GoalFileStore::new(root.join("goals")));
        let goal_result = create_goal(
            &service,
            GoalCreateParams {
                title: "Safe title".into(),
                summary: "Summary\n## Notes\nInjected".into(),
                why_it_matters: String::new(),
                priority: Some(2),
            },
        )
        .await;
        assert!(text(goal_result).contains("Markdown section headings"));
        assert!(service.list_all().await.unwrap().is_empty());
        drop(tokio::fs::remove_dir_all(root).await);
    }
}

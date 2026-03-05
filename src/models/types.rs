// ─── Conversation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub(crate) enum ConversationState {
    Pending,
    Active,
    Closed,
}

impl ConversationState {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }
}

impl std::fmt::Display for ConversationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub(crate) enum TurnRole {
    User,
    Assistant,
}

impl TurnRole {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

impl std::fmt::Display for TurnRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Goal ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub(crate) enum GoalStatus {
    Active,
    Completed,
    Archived,
    Paused,
}

impl GoalStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Archived => "archived",
            Self::Paused => "paused",
        }
    }
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub(crate) enum GoalPriority {
    High,
    Medium,
    Low,
}

impl GoalPriority {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

impl std::fmt::Display for GoalPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub(crate) enum GoalSource {
    User,
    Inferred,
}

impl GoalSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Inferred => "inferred",
        }
    }
}

impl std::fmt::Display for GoalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Memory ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum MemoryType {
    ShortTerm,
    LongTerm,
    Explicit,
}

impl MemoryType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ShortTerm => "short_term",
            Self::LongTerm => "long_term",
            Self::Explicit => "explicit",
        }
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum MemorySource {
    Observation,
    Conversation,
    VisualDiary,
    Consolidated,
}

impl MemorySource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Conversation => "conversation",
            Self::VisualDiary => "visual_diary",
            Self::Consolidated => "consolidated",
        }
    }
}

impl std::fmt::Display for MemorySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Observation ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservationSource {
    Screen,
    #[allow(dead_code)] // planned: audio input
    Audio,
    #[allow(dead_code)] // planned: clipboard monitoring
    Clipboard,
    UserMessage,
}

impl ObservationSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Audio => "audio",
            Self::Clipboard => "clipboard",
            Self::UserMessage => "user_message",
        }
    }
}

impl std::fmt::Display for ObservationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Agent Job ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgentJobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl AgentJobStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for AgentJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Goal Plan ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalPlanStatus {
    PendingApproval,
    Approved,
    AutoApproved,
    InProgress,
    Completed,
    Failed,
    Rejected,
}

impl GoalPlanStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::PendingApproval => "pending_approval",
            Self::Approved => "approved",
            Self::AutoApproved => "auto_approved",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
        }
    }
}

impl std::fmt::Display for GoalPlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalPlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl GoalPlanStepStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Skipped)
    }
}

impl std::fmt::Display for GoalPlanStepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_plan_step_status_in_progress_uses_snake_case() {
        assert_eq!(GoalPlanStepStatus::InProgress.as_str(), "in_progress");
        assert_eq!(GoalPlanStepStatus::InProgress.to_string(), "in_progress");
    }

    #[test]
    fn goal_plan_step_status_serde_round_trip() {
        let status = GoalPlanStepStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");
        let back: GoalPlanStepStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, GoalPlanStepStatus::InProgress);
    }

    #[test]
    fn goal_plan_step_status_all_variants_consistent() {
        for status in [
            GoalPlanStepStatus::Pending,
            GoalPlanStepStatus::InProgress,
            GoalPlanStepStatus::Completed,
            GoalPlanStepStatus::Failed,
            GoalPlanStepStatus::Skipped,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let from_json: GoalPlanStepStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(from_json, status);
            assert_eq!(format!("\"{status}\""), json, "as_str and serde must agree");
        }
    }

    #[test]
    fn goal_plan_status_in_progress_matches_step_convention() {
        assert_eq!(GoalPlanStatus::InProgress.as_str(), "in_progress");
        assert_eq!(GoalPlanStepStatus::InProgress.as_str(), "in_progress");
    }
}

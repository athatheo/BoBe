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

/// Goals are file-backed (`~/.bobe/goals/<id>.md`); this enum is the
/// stable wire shape the API + skill files reference. Priority is a
/// raw `u8` on `GoalDoc`, not an enum, so the agent can express finer
/// gradations without changing types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

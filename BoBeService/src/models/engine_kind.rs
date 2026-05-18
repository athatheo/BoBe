use serde::{Deserialize, Serialize};

/// Drift-locked against Swift's `EngineKind` by `scripts/check-cross-language-constants.sh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EngineKind {
    #[default]
    CopilotCloud,
    Local,
}

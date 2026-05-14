//! LLM provider config — engine type (copilot_cloud / local), per-class
//! model + reasoning effort overrides. Hot-applied via `ConfigManager`
//! with a Hard reload (engine/provider URL change) or Soft reload
//! (model/reasoning change).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct EngineConfig {
    /// `"copilot_cloud"` or `"local"`. Hot-applied via `ConfigManager`.
    pub(crate) engine: String,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_base_url: Option<String>,
    /// Alias `provider_text_model` retained for pre-foundation configs on this branch.
    #[serde(
        alias = "provider_text_model",
        deserialize_with = "empty_string_as_none",
        default
    )]
    pub(crate) provider_chat_model: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_batch_model: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_vision_model: Option<String>,
    /// Per-class reasoning effort overrides (`"low"|"medium"|"high"|...`). Only meaningful for
    /// models with non-empty `supported_reasoning_efforts`; daemon silently ignores otherwise.
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_chat_reasoning: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_batch_reasoning: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    pub(crate) provider_vision_reasoning: Option<String>,
    /// Only effective in local mode; cloud mode refuses `COPILOT_OFFLINE=true`.
    pub(crate) provider_offline: bool,
}

/// Without this, a persisted `provider_x = ""` would deserialize as `Some("")` and pass an empty model to the SDK.
fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.is_empty()))
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine: "copilot_cloud".into(),
            provider_base_url: None,
            provider_chat_model: None,
            provider_batch_model: None,
            provider_vision_model: None,
            provider_chat_reasoning: None,
            provider_batch_reasoning: None,
            provider_vision_reasoning: None,
            provider_offline: true,
        }
    }
}

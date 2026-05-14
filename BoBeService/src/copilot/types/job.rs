//! Batch job IO — input + structured output shape used by the
//! batch worker classes (Goals, Decide, Consolidate). The opaque
//! `serde_json::Value` for `input`/`output` is intentional: each
//! caller defines its own JSON schema.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobInput {
    pub(crate) job_id: Uuid,
    pub(crate) kind: String,
    pub(crate) instructions: String,
    #[serde(default)]
    pub(crate) input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobOutput {
    pub(crate) job_id: Uuid,
    #[serde(default)]
    pub(crate) output: serde_json::Value,
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

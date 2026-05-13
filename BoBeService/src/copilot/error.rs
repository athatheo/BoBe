use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkerError {
    /// Check `is_transport_failure` on the source to know if `Client` should be discarded.
    #[error("copilot SDK: {0}")]
    Sdk(#[from] github_copilot_sdk::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("worker returned no parseable JSON output for job {0}")]
    NoJsonOutput(Uuid),

    #[error("worker returned no assistant.message event for job {0}")]
    NoAssistantMessage(Uuid),
}

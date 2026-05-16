//! Crate-wide constants. Anything that appears in more than one file OR
//! crosses the Rust/Swift wire boundary lives here. Cross-language pairs
//! are locked by `scripts/check-cross-language-constants.sh`.

pub(crate) const MILLIS_PER_SECOND: f64 = 1000.0;

/// `bobe-daemon serve` port. Mirror: Swift `DaemonConfig.port`.
pub(crate) const DEFAULT_DAEMON_PORT: u16 = 8766;

/// Ollama native root (no `/v1`) for `/api/tags`, `/api/pull`.
pub(crate) const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";

/// Ollama OpenAI-compat root (with `/v1`) for chat completions when
/// engine = "local".
pub(crate) const DEFAULT_OLLAMA_V1_URL: &str = "http://127.0.0.1:11434/v1";

/// Wire-format engine kinds. Mirror: Swift `EngineKind`.
pub(crate) mod engine_kind {
    pub(crate) const COPILOT_CLOUD: &str = "copilot_cloud";
    pub(crate) const LOCAL: &str = "local";
}

/// MCP server status strings. Mirror: Swift `McpServerStatusWire`.
pub(crate) mod mcp_status {
    pub(crate) const CONNECTED: &str = "connected";
    pub(crate) const FAILED: &str = "failed";
    pub(crate) const NEEDS_AUTH: &str = "needs-auth";
    pub(crate) const PENDING: &str = "pending";
    pub(crate) const DISABLED: &str = "disabled";
    pub(crate) const NOT_CONFIGURED: &str = "not-configured";
    pub(crate) const UNKNOWN: &str = "unknown";
}

/// `~/$HOME` subdir for memory.md, conversation DB, voice models.
/// Mirror: Swift `BobePaths.dataDirName`.
pub(crate) const BOBE_DATA_DIR_NAME: &str = ".bobe";

/// Tool-call SSE status strings emitted by `util::sse::factories`.
/// Mirror: Swift `ToolCallStatusWire`.
pub(crate) mod tool_call_status {
    pub(crate) const START: &str = "start";
    pub(crate) const COMPLETE: &str = "complete";
}

/// EOU debounce (ms) for `PauseSensitivity` variants. Daemon forwards the
/// enum string; Swift `VoiceReadiness.eouDelayMs` consumes these. Rust
/// side is documentation only — drift script locks both sides.
#[allow(dead_code, reason = "drift checkpoint; Swift consumes the values")]
pub(crate) mod pause_sensitivity_ms {
    pub(crate) const TIGHT: u32 = 600;
    pub(crate) const BALANCED: u32 = 1280;
    pub(crate) const PATIENT: u32 = 2000;
}

/// Voice wire/disk constants. Mirror: Swift `VoiceWire`.
pub(crate) mod voice_wire {
    pub(crate) const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
    pub(crate) const KOKORO_MODEL_DIR: &str = "kokoro-multi-lang-v1_0";
    pub(crate) const DEFAULT_PERSONA: &str = "af_bella";
    /// Matches the snake_case serde rename of `VoiceModelKind::Tts`.
    pub(crate) const MODEL_KIND_TTS: &str = "tts";
}

//! Crate-wide constants. Define values once here when they appear in more
//! than one file or cross the Rust/Swift boundary.

pub(crate) const MILLIS_PER_SECOND: f64 = 1000.0;

/// Default port for `bobe-daemon serve`. Must match `BoBeMacUI`'s
/// `DaemonConfig.port` in Swift; see `BoBeMacUI/BoBe/App/Constants.swift`.
pub(crate) const DEFAULT_DAEMON_PORT: u16 = 8766;

/// Default Ollama server root (no `/v1` suffix). Used for the native
/// `/api/tags`, `/api/pull` endpoints.
pub(crate) const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";

/// Default Ollama OpenAI-compat root (with `/v1`). Used as the
/// `provider_base_url` for chat completions when engine = "local".
pub(crate) const DEFAULT_OLLAMA_V1_URL: &str = "http://127.0.0.1:11434/v1";

/// Wire-format engine kinds. Daemon `config.engine.engine` + the
/// `SettingsResponse.engine` field serialize as these strings; Swift
/// `Codable` decodes against them. Match the values in
/// `BoBeMacUI/BoBe/Models/BobeTypes.swift::EngineKind`.
pub(crate) mod engine_kind {
    pub(crate) const COPILOT_CLOUD: &str = "copilot_cloud";
    pub(crate) const LOCAL: &str = "local";
}

/// Wire-format MCP server status strings. Emitted by
/// `services::mcp_config_service::build_server_summary`; consumed by the
/// Swift `MCPServersPanel.statusBadge` switch. Match the values in
/// `BoBeMacUI/BoBe/App/Constants.swift::McpServerStatusWire`.
pub(crate) mod mcp_status {
    pub(crate) const CONNECTED: &str = "connected";
    pub(crate) const FAILED: &str = "failed";
    pub(crate) const NEEDS_AUTH: &str = "needs-auth";
    pub(crate) const PENDING: &str = "pending";
    pub(crate) const DISABLED: &str = "disabled";
    pub(crate) const NOT_CONFIGURED: &str = "not-configured";
    pub(crate) const UNKNOWN: &str = "unknown";
}

/// Voice-pipeline wire/disk constants shared across Rust + Swift.
/// Daemon emits TTS frames at this sample rate; Swift `AVAudioPlayerNode`
/// must match. Install script downloads the Kokoro tarball, extracts to
/// the named subdir of `~/.bobe/models/`; the daemon loader + Swift voice
/// settings UI must agree on that path. Persona is the default Kokoro
/// voice id when the user hasn't picked one in settings.
/// Match `BoBeMacUI/BoBe/App/Constants.swift::VoiceWire`.
/// Bobe data directory name relative to `$HOME`. The daemon writes
/// memory.md, conversation DB, voice models, etc. under here; the Swift
/// app constructs the same path to find/launch the daemon binary and to
/// expose the location to the user. Match `BoBeMacUI/BoBe/App/Constants.
/// swift::BobePaths.dataDirName`.
pub(crate) const BOBE_DATA_DIR_NAME: &str = ".bobe";

/// Tool-call SSE status discriminator strings. Emitted by
/// `util::sse::factories::{tool_call_start_event, tool_call_complete_event}`;
/// consumed by Swift `Stores::ToolExecutionController` via the payload
/// `status` field. Match `BoBeMacUI/BoBe/App/Constants.swift::ToolCallStatusWire`.
pub(crate) mod tool_call_status {
    pub(crate) const START: &str = "start";
    pub(crate) const COMPLETE: &str = "complete";
}

/// EOU debounce values for `PauseSensitivity` variants. The daemon ships
/// the enum string; Swift `Voice::VoiceReadiness::eouDelayMs` maps each
/// to one of these millisecond values and hands it to FluidAudio's
/// Parakeet (built-in EOU debounce) or the Qwen3 wrapper's silence
/// timer. Defined on the Rust side as the canonical contract so the
/// drift script catches anyone moving a value out of lockstep with the
/// Swift `PauseSensitivityMs` mirror.
#[allow(
    dead_code,
    reason = "Documentation + drift checkpoint; the daemon forwards the enum string and never reads the ms — Swift does."
)]
pub(crate) mod pause_sensitivity_ms {
    pub(crate) const TIGHT: u32 = 600;
    pub(crate) const BALANCED: u32 = 1280;
    pub(crate) const PATIENT: u32 = 2000;
}

pub(crate) mod voice_wire {
    pub(crate) const TTS_OUTPUT_SAMPLE_RATE: u32 = 24_000;
    pub(crate) const KOKORO_MODEL_DIR: &str = "kokoro-multi-lang-v1_0";
    pub(crate) const DEFAULT_PERSONA: &str = "af_bella";
    /// Wire value for the TTS install model kind. Match the
    /// `#[serde(rename_all = "snake_case")]`-serialized variant of
    /// `voice::install_artifacts::VoiceModelKind::Tts`.
    pub(crate) const MODEL_KIND_TTS: &str = "tts";
}

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

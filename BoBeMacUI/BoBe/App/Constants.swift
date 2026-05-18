import Foundation

// Cross-language pairs are locked by check-cross-language-constants.sh.

enum DaemonConfig {
    static let host = "127.0.0.1"
    /// Mirror: Rust `DEFAULT_DAEMON_PORT`.
    static let port = 8766
    static let baseURL = "http://\(host):\(port)"
}

enum OllamaDefaults {
    static let baseURL = "http://127.0.0.1:11434"
    static let v1URL = "http://127.0.0.1:11434/v1"
}

/// Mirror: Rust `engine_kind::*`.
enum EngineKind {
    static let copilotCloud = "copilot_cloud"
    static let local = "local"
}

/// `~/$HOME` subdir. Mirror: Rust `BOBE_DATA_DIR_NAME`. `$BOBE_DATA_DIR`
/// env override on the Rust side; Swift mirrors only the relative name.
enum BobePaths {
    static let dataDirName = ".bobe"
}

/// Mirror: Rust `tool_call_status::*`.
enum ToolCallStatusWire {
    static let start = "start"
    static let complete = "complete"
}

/// EOU debounce ms per `voice.pause_sensitivity`. Mirror: Rust
/// `pause_sensitivity_ms::*`.
///
/// Values are tuned for the FluidAudio Parakeet streaming EOU model, which
/// already does linguistic + acoustic end-of-turn prediction before the
/// debounce timer starts — the debounce is just a safety wait for trailing
/// silence. The previous 1280ms balanced default (the model's published
/// reference value) felt sluggish in practice; 800ms is the FluidAudio
/// reference VAD-EOU value used by the Qwen3 path and matches what shipping
/// voice agents like Whisper-based pipelines use.
enum PauseSensitivityMs {
    static let tight: Int = 400
    static let balanced: Int = 800
    static let patient: Int = 1500
}

/// Wire form for both `/voice/install/status` and `/local-runtime/status`.
/// Mirror: Rust `InstallStatus` (serde snake_case).
enum InstallStatusWire: String, Codable, Sendable, Equatable {
    case idle
    case running
    case complete
    case canceled
    case failed
}

/// Mirror: Rust `voice_wire::*`.
enum VoiceWire {
    static let ttsOutputSampleRate = 24_000
    static let kokoroModelDir = "kokoro-multi-lang-v1_0"
    static let defaultPersona = "af_bella"
    static let modelKindTts = "tts"
    /// WS subprotocol — advertised in the client's Sec-WebSocket-Protocol
    /// header, echoed by the daemon. Reserves the rev hook for a future v2.
    static let subprotocolV1 = "bobe.voice.v1"
}

/// Mirror: Rust `mcp_status::*`. MCPServersPanel.statusBadge has a
/// `.default` fallback for unknown future statuses.
enum McpServerStatusWire {
    static let connected = "connected"
    static let failed = "failed"
    static let needsAuth = "needs-auth"
    static let pending = "pending"
    static let disabled = "disabled"
    static let notConfigured = "not-configured"
    static let unknown = "unknown"
}

enum WindowSizes {
    static let widthCollapsed: CGFloat = 184
    static let widthExpanded: CGFloat = 540
    static let heightCollapsed: CGFloat = 196
    static let heightAvatar: CGFloat = 180
    static let heightInput: CGFloat = 70
    static let heightExpandedChrome: CGFloat = 56
    static let heightChatViewportMin: CGFloat = 48
    static let heightChatViewportMax: CGFloat = 560
    static let heightMax: CGFloat = 900
    static let margin: CGFloat = 16
}

enum StoreTiming {
    static let textDeltaFlushMilliseconds = 150
    static let lastMessageClearSeconds: TimeInterval = 30
    static let toolCompletionLingerSeconds: TimeInterval = 5
    static let conversationClearSeconds: TimeInterval = 3
    static let captureRetryBaseMilliseconds = 350
    /// How long the SSE may be disconnected before the "Reconnecting…"
    /// label appears. Sized to absorb the typical sub-second instant
    /// reconnect after a transient drop so the user never sees flicker.
    static let reconnectStatusDelayMilliseconds = 1500
}

enum InactivityTiming {
    static let timeoutSeconds: TimeInterval = 10 * 60
    static let checkIntervalSeconds: TimeInterval = 30
}

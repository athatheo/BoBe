import Foundation

enum DaemonConfig {
    static let host = "127.0.0.1"
    /// Must match Rust `constants::DEFAULT_DAEMON_PORT`. If you change one,
    /// change the other — a CI assertion will eventually catch this drift.
    static let port = 8766
    static let baseURL = "http://\(host):\(port)"
}

/// Defaults shared with the Rust daemon's `constants` module.
enum OllamaDefaults {
    /// Native Ollama API root (no `/v1`). For `/api/tags`, `/api/pull`.
    static let baseURL = "http://127.0.0.1:11434"
    /// OpenAI-compat root (with `/v1`). For chat completions when
    /// `engine == EngineKind.local`.
    static let v1URL = "http://127.0.0.1:11434/v1"
}

/// Wire-format engine kinds. Settings round-trip strings; both sides must
/// agree. Matches the Rust `constants::engine_kind::*` consts.
enum EngineKind {
    static let copilotCloud = "copilot_cloud"
    static let local = "local"
}

/// Voice-pipeline wire/disk constants shared with the daemon. Match
/// Rust `constants::voice_wire::*`. The TTS sample rate is the
/// `AVAudioPlayerNode` playback rate AND the Hello-handshake claim sent
/// to the daemon (mismatch → rate_mismatch close). The Kokoro dir name
/// is the on-disk path Settings → Voice → Models displays as the user's
/// model location. The persona default is the fallback voice id when
/// Settings haven't been touched yet.
/// BoBe data directory name relative to `$HOME`. Match Rust
/// `constants::BOBE_DATA_DIR_NAME`. The daemon's `paths::bobe_data_dir`
/// allows `$BOBE_DATA_DIR` to override; Swift mirrors only the relative
/// name since BackendService respects the same env var via the spawned
/// daemon's env.
enum BobePaths {
    static let dataDirName = ".bobe"
}

/// Tool-call SSE status strings emitted by the daemon's
/// `util::sse::factories::tool_call_*_event`. Match Rust
/// `constants::tool_call_status::*`.
enum ToolCallStatusWire {
    static let start = "start"
    static let complete = "complete"
}

/// Install-progress status emitted by both `/voice/install/status` and
/// `/local-runtime/status`. Mirrors Rust `voice::install_artifacts::
/// InstallStatus` (serde rename_all = "snake_case") and the manual
/// mapping in `api::handlers::local_runtime::InstallSnapshotDto`. Both
/// daemon-side enums emit the same 5 strings; if they diverge in the
/// future, split this into two Swift enums.
enum InstallStatusWire: String, Codable, Sendable, Equatable {
    case idle
    case running
    case complete
    case canceled
    case failed
}

enum VoiceWire {
    static let ttsOutputSampleRate = 24_000
    static let kokoroModelDir = "kokoro-multi-lang-v1_0"
    static let defaultPersona = "af_bella"
    /// Wire value for the TTS install model kind (matches Rust
    /// `voice_wire::MODEL_KIND_TTS`). `installStatus.models` from the
    /// daemon arrives as untyped String; Settings → Voice filters by
    /// this value.
    static let modelKindTts = "tts"
}

/// Wire-format MCP server status strings emitted by the daemon's
/// `services::mcp_config_service`. Match the Rust `constants::mcp_status`
/// consts. A typed `Codable` enum would be the next-level fix; the
/// `MCPServersPanel.statusBadge` switch wants a `.default` fallback so
/// unknown future statuses still render a generic badge.
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
    static let reconnectStatusDelayMilliseconds = 600
}

enum InactivityTiming {
    static let timeoutSeconds: TimeInterval = 10 * 60
    static let checkIntervalSeconds: TimeInterval = 30
}

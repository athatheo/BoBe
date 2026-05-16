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

import Darwin
import Foundation

// Cross-language pairs are locked by check-cross-language-constants.sh.

enum DaemonEndpoint: Sendable {
    case managedLocal
    case remote(baseURL: URL, bearerToken: String)

    static func load(environment: [String: String] = ProcessInfo.processInfo.environment) -> Self {
        guard let rawURL = environment["BOBE_DAEMON_URL"], !rawURL.isEmpty else {
            return .managedLocal
        }
        guard let url = URL(string: rawURL),
              url.scheme?.lowercased() == "https",
              let host = url.host,
              !host.isEmpty,
              !Self.isLoopbackHost(host),
              url.user == nil,
              url.password == nil,
              url.query == nil,
              url.fragment == nil,
              let token = environment["BOBE_DAEMON_TOKEN"],
              !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            fatalError("Remote BOBE_DAEMON_URL requires an https non-loopback URL and BOBE_DAEMON_TOKEN")
        }
        return .remote(baseURL: url, bearerToken: token)
    }

    var baseURL: URL {
        switch self {
        case .managedLocal:
            return URL(string: "http://127.0.0.1:\(DaemonConfig.defaultPort)")
                ?? URL(fileURLWithPath: "/")
        case let .remote(baseURL, _):
            return baseURL
        }
    }

    var bearerToken: String? {
        guard case let .remote(_, token) = self else { return nil }
        return token
    }

    var managesLocalProcess: Bool {
        if case .managedLocal = self { return true }
        return false
    }

    func authorize(_ request: inout URLRequest) {
        guard let token = self.bearerToken else { return }
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }

    static func isLoopbackHost(_ host: String) -> Bool {
        let normalized = host.lowercased()
        if normalized == "localhost" || normalized == "::1" {
            return true
        }
        var address = in_addr()
        let parsed = normalized.withCString {
            inet_pton(AF_INET, $0, &address)
        }
        guard parsed == 1 else { return false }
        return withUnsafeBytes(of: &address) { bytes in
            bytes.first == 127
        }
    }
}

enum DaemonConfig {
    static let defaultHost = "127.0.0.1"
    /// Mirror: Rust `DEFAULT_DAEMON_PORT`.
    static let defaultPort = 8766
    static let endpoint = DaemonEndpoint.load()
    static let baseURL = endpoint.baseURL.absoluteString
}

enum PrivacyWire {
    /// Mirror: Rust `constants::privacy::CLIENT_TIMEOUT_SECS`.
    static let purgeRequestTimeoutSeconds: TimeInterval = 60
}

struct BodyAdapterConfiguration: Sendable {
    let baseURL: URL
    let bearerToken: String
}

enum BodyAdapterConfig {
    static func load(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> BodyAdapterConfiguration? {
        guard environment["BOBE_BODY_ADAPTER"] == "1" else { return nil }
        let rawURL = environment["BOBE_BODY_ADAPTER_URL"] ?? "http://127.0.0.1:8768"
        guard let baseURL = URL(string: rawURL),
              baseURL.scheme?.lowercased() == "http",
              baseURL.host.map(DaemonEndpoint.isLoopbackHost) == true,
              baseURL.user == nil,
              baseURL.password == nil,
              baseURL.query == nil,
              baseURL.fragment == nil,
              let token = environment["BOBE_BODY__ADAPTER_TOKEN"],
              !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            return nil
        }
        return BodyAdapterConfiguration(baseURL: baseURL, bearerToken: token)
    }
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
/// reference VAD-EOU value used by the Nemotron path and matches what shipping
/// voice agents like Whisper-based pipelines use.
enum PauseSensitivityMs {
    static let tight: Int = 400
    static let balanced: Int = 800
    static let patient: Int = 1500
}

enum VoiceSttTuning {
    /// Nemotron's trained 1.12s tier halves multilingual partial latency from
    /// the 2.24s throughput default without using the most aggressive 560ms tier.
    static let nemotronChunkMs = 1_120
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
    static let widthComposer: CGFloat = 430
    static let widthExpanded: CGFloat = 540
    static let heightCollapsed: CGFloat = 196
    static let heightAvatar: CGFloat = 180
    static let heightInput: CGFloat = 70
    static let heightExpandedChrome: CGFloat = 56
    static let heightChatViewportMin: CGFloat = 48
    static let heightChatViewportMax: CGFloat = 560
    static let heightAmbientMessageViewport: CGFloat = 240
    static let heightAmbientAnswer: CGFloat = 500
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

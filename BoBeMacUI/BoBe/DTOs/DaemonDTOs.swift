import Foundation

// MARK: - Auth + models DTOs

struct AuthStatusResponse: Codable, Sendable {
    /// The daemon's `/auth/status` payload also includes `host` and
    /// `cli_version`, neither of which is surfaced in the UI today, so
    /// they're omitted from this Codable mirror. JSON decode silently
    /// ignores unknown fields, so the wire shape can grow without
    /// breaking this struct.
    let isAuthenticated: Bool
    let authType: String?
    let login: String?
    let statusMessage: String?
    /// `nil` if the bundled-CLI feature is disabled or not yet extracted.
    let cliPath: String?

    enum CodingKeys: String, CodingKey {
        case isAuthenticated = "is_authenticated"
        case authType = "auth_type"
        case login
        case statusMessage = "status_message"
        case cliPath = "cli_path"
    }
}

struct ListModelsResponse: Codable, Sendable {
    let engine: String
    let models: [ModelInfo]
    /// `"upstream"` when the SDK / Ollama answered, `"fallback"` when the
    /// daemon returned a static catalog because the upstream call failed.
    /// Defaulted for backward compatibility with older daemons.
    let source: String
    /// Optional non-blocking notice about why the list might not reflect the
    /// user's real entitlements (auth needed, plan empty, transport error).
    /// The UI renders this as an inline banner above the picker; it never
    /// blocks selection.
    let notice: ModelsNotice?

    enum CodingKeys: String, CodingKey {
        case engine, models, source, notice
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.engine = try c.decode(String.self, forKey: .engine)
        self.models = try c.decode([ModelInfo].self, forKey: .models)
        self.source = try c.decodeIfPresent(String.self, forKey: .source) ?? "upstream"
        self.notice = try c.decodeIfPresent(ModelsNotice.self, forKey: .notice)
    }

    init(engine: String, models: [ModelInfo], source: String = "upstream", notice: ModelsNotice? = nil) {
        self.engine = engine
        self.models = models
        self.source = source
        self.notice = notice
    }
}

/// Structured non-blocking warning attached to a `/models` response. The
/// `code` field is stable (`AUTH_REQUIRED` / `NO_ENTITLEMENTS` /
/// `SDK_ERROR` / `UPSTREAM_UNREACHABLE`); the `message` is free-form daemon
/// detail kept for forensics.
struct ModelsNotice: Codable, Equatable {
    let code: String
    let message: String
}

struct ModelInfo: Codable, Sendable, Identifiable, Hashable {
    let id: String
    let name: String
    let vision: Bool
    /// `nil` for Ollama — `/api/tags` doesn't expose context window.
    let contextWindow: Int?
    /// Billing cost relative to base rate. `nil` for Ollama; `0.0` = free tier.
    let multiplier: Double?
    let defaultReasoningEffort: String?
    let supportedReasoningEfforts: [String]
    /// `"enabled"` / `"disabled"` / `"unconfigured"` — UI dims non-enabled entries.
    let policyState: String?

    var supportsReasoningEffort: Bool {
        !self.supportedReasoningEfforts.isEmpty
    }

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case vision
        case contextWindow = "context_window"
        case multiplier
        case defaultReasoningEffort = "default_reasoning_effort"
        case supportedReasoningEfforts = "supported_reasoning_efforts"
        case policyState = "policy_state"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.id = try c.decode(String.self, forKey: .id)
        self.name = try c.decode(String.self, forKey: .name)
        self.vision = try c.decode(Bool.self, forKey: .vision)
        self.contextWindow = try c.decodeIfPresent(Int.self, forKey: .contextWindow)
        self.multiplier = try c.decodeIfPresent(Double.self, forKey: .multiplier)
        self.defaultReasoningEffort = try c.decodeIfPresent(String.self, forKey: .defaultReasoningEffort)
        self.supportedReasoningEfforts = try c.decodeIfPresent([String].self, forKey: .supportedReasoningEfforts) ?? []
        self.policyState = try c.decodeIfPresent(String.self, forKey: .policyState)
    }
}

// MARK: - Local-runtime install DTOs

struct LocalRuntimeInstallRequest: Codable, Sendable {
    let chatModel: String
    let batchModel: String
    let visionModel: String

    enum CodingKeys: String, CodingKey {
        case chatModel = "chat_model"
        case batchModel = "batch_model"
        case visionModel = "vision_model"
    }
}

struct LocalRuntimeMessageResponse: Codable, Sendable {
    let message: String
}

struct LocalRuntimeSnapshot: Codable, Sendable {
    let status: InstallStatusWire
    let error: String?
    let runtime: LocalRuntimeDownload
    let chatModel: LocalRuntimePull
    let batchModel: LocalRuntimePull
    let visionModel: LocalRuntimePull

    enum CodingKeys: String, CodingKey {
        case status
        case error
        case runtime
        case chatModel = "chat_model"
        case batchModel = "batch_model"
        case visionModel = "vision_model"
    }
}

struct LocalRuntimeDownload: Codable, Sendable {
    let currentBytes: UInt64
    let totalBytes: UInt64?
    let percent: Int?

    enum CodingKeys: String, CodingKey {
        case currentBytes = "current_bytes"
        case totalBytes = "total_bytes"
        case percent
    }
}

struct LocalRuntimePull: Codable, Sendable {
    let status: String
    let completedBytes: UInt64?
    let totalBytes: UInt64?
    let percent: Int?

    enum CodingKeys: String, CodingKey {
        case status
        case completedBytes = "completed_bytes"
        case totalBytes = "total_bytes"
        case percent
    }
}

// MARK: - Voice install DTOs

struct VoiceInstallSnapshot: Codable, Sendable {
    let status: InstallStatusWire
    let error: String?
    let models: [VoiceModelProgress]
    let installed: VoiceInstallPresence

    /// True when the daemon reports an active install. Matches the Rust
    /// `InstallStatus::Running` variant's wire form.
    var isRunning: Bool {
        self.status == .running
    }

    /// Convenience for the wizard step's continue button.
    var isComplete: Bool {
        self.status == .complete
    }

    var isTerminal: Bool {
        [.complete, .canceled, .failed, .idle].contains(self.status)
    }
}

struct VoiceModelProgress: Codable, Sendable, Identifiable {
    let kind: String
    let label: String
    let status: String
    let bytesDownloaded: UInt64
    let bytesTotal: UInt64?
    let percent: Int?

    var id: String {
        self.kind
    }

    enum CodingKeys: String, CodingKey {
        case kind
        case label
        case status
        case bytesDownloaded = "bytes_downloaded"
        case bytesTotal = "bytes_total"
        case percent
    }
}

/// Daemon-side install presence. Mirrors PresenceSnapshot in
/// BoBeService/src/api/handlers/voice_install.rs (Mode B: TTS-only).
struct VoiceInstallPresence: Codable, Sendable {
    let tts: Bool
    let allPresent: Bool

    enum CodingKeys: String, CodingKey {
        case tts
        case allPresent = "all_present"
    }
}

// MARK: - Copilot login DTOs

/// Mirrors `LoginPhase` in BoBeService/src/copilot/login.rs. The Rust
/// side uses `#[serde(tag = "phase")]` so each variant is a flat JSON
/// object with a `phase` discriminator field.
enum CopilotLoginPhase: Equatable {
    case preparing
    case awaitingUser(url: String, code: String)
    case polling(url: String, code: String)
    case completed
    case failed(message: String)
    case canceled
    case unsupported(phase: String)

    var isTerminal: Bool {
        switch self {
        case .completed, .failed, .canceled, .unsupported: true
        default: false
        }
    }
}

extension CopilotLoginPhase: Decodable {
    private enum CodingKeys: String, CodingKey {
        case phase
        case url
        case code
        case message
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let phase = try container.decode(String.self, forKey: .phase)
        switch phase {
        case "preparing":
            self = .preparing
        case "awaiting_user":
            self = try .awaitingUser(
                url: container.decode(String.self, forKey: .url),
                code: container.decode(String.self, forKey: .code)
            )
        case "polling":
            self = try .polling(
                url: container.decode(String.self, forKey: .url),
                code: container.decode(String.self, forKey: .code)
            )
        case "completed":
            self = .completed
        case "failed":
            self = try .failed(message: container.decode(String.self, forKey: .message))
        case "canceled":
            self = .canceled
        default:
            self = .unsupported(phase: phase)
        }
    }
}

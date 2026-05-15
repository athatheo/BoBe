import Foundation

// MARK: - Auth + models DTOs

struct AuthStatusResponse: Codable, Sendable {
    let isAuthenticated: Bool
    let authType: String?
    let host: String?
    let login: String?
    let statusMessage: String?
    /// `nil` if the bundled-CLI feature is disabled or not yet extracted.
    let cliPath: String?
    let cliVersion: String?

    enum CodingKeys: String, CodingKey {
        case isAuthenticated = "is_authenticated"
        case authType = "auth_type"
        case host
        case login
        case statusMessage = "status_message"
        case cliPath = "cli_path"
        case cliVersion = "cli_version"
    }
}

struct ListModelsResponse: Codable, Sendable {
    let engine: String
    let models: [ModelInfo]
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

    var supportsReasoningEffort: Bool { !self.supportedReasoningEfforts.isEmpty }

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
    let status: String
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
    let status: String
    let models: [VoiceModelProgress]
    let installed: VoiceInstallPresence

    /// True when the daemon reports an active install. Matches the Rust
    /// `InstallStatus::Running` variant's wire form.
    var isRunning: Bool { self.status == "running" }
    /// Convenience for the wizard step's continue button.
    var isComplete: Bool { self.status == "complete" }
    var isTerminal: Bool { ["complete", "canceled", "failed", "idle"].contains(self.status) }
}

struct VoiceModelProgress: Codable, Sendable, Identifiable {
    let kind: String
    let label: String
    let status: String
    let bytesDownloaded: UInt64
    let bytesTotal: UInt64?
    let percent: Int?

    var id: String { self.kind }

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

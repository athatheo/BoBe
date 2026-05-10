import Foundation

// MARK: - Settings

/// Mirrors the daemon's `/settings` GET response. Capture/checkin/
/// conversation/goal-cadence/mcp are the runtime knobs; the engine
/// section selects whether the Copilot CLI subprocess drives GitHub
/// Copilot's cloud or a user-managed local OpenAI-compat server
/// (typically a managed Ollama).
struct DaemonSettings: Codable, Sendable {
    var captureEnabled: Bool
    var captureIntervalSeconds: Int
    var checkinEnabled: Bool
    var checkinTimes: [String]
    var checkinJitterMinutes: Int
    var conversationInactivityTimeoutSeconds: Int
    var conversationAutoCloseMinutes: Int
    var goalCheckIntervalSeconds: Double
    var mcpEnabled: Bool
    /// "copilot_cloud" (default) or "local". Restart-required.
    var engine: String
    var providerBaseUrl: String?
    var providerTextModel: String?
    var providerVisionModel: String?
    var providerOffline: Bool

    enum CodingKeys: String, CodingKey {
        case captureEnabled = "capture_enabled"
        case captureIntervalSeconds = "capture_interval_seconds"
        case checkinEnabled = "checkin_enabled"
        case checkinTimes = "checkin_times"
        case checkinJitterMinutes = "checkin_jitter_minutes"
        case conversationInactivityTimeoutSeconds = "conversation_inactivity_timeout_seconds"
        case conversationAutoCloseMinutes = "conversation_auto_close_minutes"
        case goalCheckIntervalSeconds = "goal_check_interval_seconds"
        case mcpEnabled = "mcp_enabled"
        case engine
        case providerBaseUrl = "provider_base_url"
        case providerTextModel = "provider_text_model"
        case providerVisionModel = "provider_vision_model"
        case providerOffline = "provider_offline"
    }
}

/// All-optional payload for PATCH /settings. Server only applies fields
/// that are present.
struct SettingsUpdateRequest: Codable, Sendable {
    var captureEnabled: Bool?
    var captureIntervalSeconds: Int?
    var checkinEnabled: Bool?
    var checkinTimes: [String]?
    var checkinJitterMinutes: Int?
    var conversationInactivityTimeoutSeconds: Int?
    var conversationAutoCloseMinutes: Int?
    var goalCheckIntervalSeconds: Double?
    var mcpEnabled: Bool?
    var engine: String?
    var providerBaseUrl: String?
    var providerTextModel: String?
    var providerVisionModel: String?
    var providerOffline: Bool?

    enum CodingKeys: String, CodingKey {
        case captureEnabled = "capture_enabled"
        case captureIntervalSeconds = "capture_interval_seconds"
        case checkinEnabled = "checkin_enabled"
        case checkinTimes = "checkin_times"
        case checkinJitterMinutes = "checkin_jitter_minutes"
        case conversationInactivityTimeoutSeconds = "conversation_inactivity_timeout_seconds"
        case conversationAutoCloseMinutes = "conversation_auto_close_minutes"
        case goalCheckIntervalSeconds = "goal_check_interval_seconds"
        case mcpEnabled = "mcp_enabled"
        case engine
        case providerBaseUrl = "provider_base_url"
        case providerTextModel = "provider_text_model"
        case providerVisionModel = "provider_vision_model"
        case providerOffline = "provider_offline"
    }
}

struct SettingsUpdateResponse: Codable, Sendable {
    let message: String
    let appliedFields: [String]
    let restartRequiredFields: [String]
    /// Daemon flips this to `true` when the in-memory swap succeeded but
    /// writing to ~/.bobe/config.toml failed. Settings are live for this
    /// session but won't survive a restart — surface to the user.
    var persistFailed: Bool?

    enum CodingKeys: String, CodingKey {
        case message
        case appliedFields = "applied_fields"
        case restartRequiredFields = "restart_required_fields"
        case persistFailed = "persist_failed"
    }
}

// MARK: - Message / Health

struct SendMessageRequest: Codable, Sendable {
    let content: String
}

struct SendMessageResponse: Codable, Sendable {
    let messageId: String

    enum CodingKeys: String, CodingKey {
        case messageId = "message_id"
    }
}

/// `/health` body — the daemon always returns 200 even when the DB is
/// in error, so callers must inspect `services.database` rather than
/// the HTTP status.
struct HealthResponse: Codable, Sendable {
    let status: String
    let version: String?
    let services: HealthServices?
}

struct HealthServices: Codable, Sendable {
    let database: String
}

/// `/status` body — runtime snapshot used to seed local state on SSE
/// reconnect. `accepting_user_messages` is the daemon's authoritative
/// answer to "can the user send right now?"; UI should mirror it.
struct StatusResponse: Codable, Sendable {
    let indicator: String
    let capturing: Bool
    let acceptingUserMessages: Bool
    let version: String?

    enum CodingKeys: String, CodingKey {
        case indicator, capturing, version
        case acceptingUserMessages = "accepting_user_messages"
    }

    var indicatorType: IndicatorType {
        IndicatorType(rawValue: self.indicator) ?? .unknown
    }
}

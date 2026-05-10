import Foundation

// MARK: - Settings

/// Engine fields hot-swap: PATCH triggers daemon worker registry reload.
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
    var engine: String
    var providerBaseUrl: String?
    var providerChatModel: String?
    var providerBatchModel: String?
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
        case providerChatModel = "provider_chat_model"
        case providerBatchModel = "provider_batch_model"
        case providerVisionModel = "provider_vision_model"
        case providerOffline = "provider_offline"
    }
}

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
    var providerChatModel: String?
    var providerBatchModel: String?
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
        case providerChatModel = "provider_chat_model"
        case providerBatchModel = "provider_batch_model"
        case providerVisionModel = "provider_vision_model"
        case providerOffline = "provider_offline"
    }
}

struct SettingsUpdateResponse: Codable, Sendable {
    let message: String
    let appliedFields: [String]
    let restartRequiredFields: [String]
    /// `true` = in-memory swap OK but writing config.toml failed; won't survive restart.
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

/// Always returns 200 even on DB error — inspect `services.database`, not status.
struct HealthResponse: Codable, Sendable {
    let status: String
    let version: String?
    let services: HealthServices?
}

struct HealthServices: Codable, Sendable {
    let database: String
}

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

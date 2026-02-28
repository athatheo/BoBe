import Foundation

// MARK: - SSE Event Stream Types

/// SSE event types from GET /events
enum EventType: String, Codable, Sendable {
    case indicator
    case textDelta = "text_delta"
    case toolCall = "tool_call"
    case toolCallStart = "tool_call_start"
    case toolCallComplete = "tool_call_complete"
    case error
    case heartbeat
    case endOfTurn = "end_of_turn"
    case conversationClosed = "conversation_closed"
    case unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = EventType(rawValue: raw) ?? .unknown
    }
}

/// All SSE events wrapped in this envelope
struct StreamBundle: Codable, Sendable {
    let type: EventType
    let payload: AnyCodablePayload
    let messageId: String
    let timestamp: String
    let description: String

    enum CodingKeys: String, CodingKey {
        case type
        case payload
        case messageId = "message_id"
        case timestamp
        case description
    }
}

/// Indicator event payload
struct IndicatorPayload: Codable, Sendable {
    let indicator: IndicatorType
    let message: String?
    let progress: Double?
}

/// Text delta payload for streaming responses
struct TextDeltaPayload: Codable, Sendable {
    let delta: String
    let sequence: Int
    let done: Bool
}

/// Tool call start payload
struct ToolCallStartPayload: Codable, Sendable {
    let status: String // "start"
    let toolName: String
    let toolCallId: String

    enum CodingKeys: String, CodingKey {
        case status
        case toolName = "tool_name"
        case toolCallId = "tool_call_id"
    }
}

/// Tool call complete payload
struct ToolCallCompletePayload: Codable, Sendable {
    let status: String // "complete"
    let toolName: String
    let toolCallId: String
    let success: Bool
    let error: String?
    let durationMs: Int?

    enum CodingKeys: String, CodingKey {
        case status
        case toolName = "tool_name"
        case toolCallId = "tool_call_id"
        case success
        case error
        case durationMs = "duration_ms"
    }
}

/// Error event payload
struct ErrorPayload: Codable, Sendable {
    let code: String
    let message: String
    let recoverable: Bool
    let details: [String: AnyCodableValue]?
}

/// Conversation closed payload
struct ConversationClosedPayload: Codable, Sendable {
    let conversationId: String
    let reason: String
    let turnCount: Int

    enum CodingKeys: String, CodingKey {
        case conversationId = "conversation_id"
        case reason
        case turnCount = "turn_count"
    }
}

// MARK: - Flexible JSON Payload Handling

/// A type-erased Codable wrapper for SSE payloads (decoded lazily by type)
struct AnyCodablePayload: Codable, Sendable {
    let data: Data

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        // Capture the raw JSON for the payload
        if let dict = try? container.decode([String: AnyCodableValue].self) {
            self.data = try JSONEncoder().encode(dict)
        } else {
            self.data = Data()
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(data)
    }

    func decode<T: Decodable>(as type: T.Type) throws -> T {
        try JSONDecoder().decode(type, from: data)
    }
}

/// Type-erased JSON value for flexible payload handling
enum AnyCodableValue: Codable, Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
    case array([AnyCodableValue])
    case dictionary([String: AnyCodableValue])

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let v = try? container.decode(Bool.self) {
            self = .bool(v)
        } else if let v = try? container.decode(Int.self) {
            self = .int(v)
        } else if let v = try? container.decode(Double.self) {
            self = .double(v)
        } else if let v = try? container.decode(String.self) {
            self = .string(v)
        } else if let v = try? container.decode([AnyCodableValue].self) {
            self = .array(v)
        } else if let v = try? container.decode([String: AnyCodableValue].self) {
            self = .dictionary(v)
        } else if container.decodeNil() {
            self = .null
        } else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath,
                      debugDescription: "Unsupported JSON value"))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let v): try container.encode(v)
        case .int(let v): try container.encode(v)
        case .double(let v): try container.encode(v)
        case .bool(let v): try container.encode(v)
        case .null: try container.encodeNil()
        case .array(let v): try container.encode(v)
        case .dictionary(let v): try container.encode(v)
        }
    }
}

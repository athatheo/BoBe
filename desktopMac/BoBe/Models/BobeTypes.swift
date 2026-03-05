import Foundation

// MARK: - Core State Types

/// Priority: loading > speaking > thinking > wants_to_speak > capturing > idle
enum BobeStateType: String, Sendable, Equatable {
    case loading
    case error
    case idle
    case capturing
    case thinking
    case speaking
    case wantsToSpeak = "wants_to_speak"
    case shuttingDown = "shutting_down"
}

/// Matches Rust SCREAMING_SNAKE_CASE indicator variants.
enum IndicatorType: String, Codable, Sendable, Equatable {
    case idle = "IDLE"
    case screenCapture = "SCREEN_CAPTURE"
    case thinking = "THINKING"
    case toolCalling = "TOOL_CALLING"
    case streaming = "STREAMING"
    case unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = IndicatorType(rawValue: raw) ?? .unknown
    }
}

enum MessageSender: String, Sendable {
    case user
    case bobe
}

struct ChatMessage: Identifiable, Sendable {
    let id: String
    let sender: MessageSender
    var content: String
    let timestamp: Date
    var isStreaming: Bool
    var isPending: Bool

    init(
        id: String = UUID().uuidString,
        sender: MessageSender,
        content: String,
        timestamp: Date = .now,
        isStreaming: Bool = false,
        isPending: Bool = false
    ) {
        self.id = id
        self.sender = sender
        self.content = content
        self.timestamp = timestamp
        self.isStreaming = isStreaming
        self.isPending = isPending
    }
}

struct ToolExecution: Identifiable, Sendable {
    var id: String {
        self.toolCallId
    }

    let toolName: String
    let toolCallId: String
    var status: ToolExecutionStatus
    var error: String?
    var durationMs: Int?
    let startedAt: Date
    var completedAt: Date?
}

enum ToolExecutionStatus: String, Sendable {
    case running
    case success
    case error
}

struct BobeContext: Sendable {
    var daemonConnected = false
    var daemonError = false
    var capturing = false
    var captureInProgress = false
    var thinking = false
    var speaking = false
    var shuttingDown = false
    var lastMessage: String?
    var errorMessage: String?
    var currentMessage = ""
    var currentMessageId: String?
    var messages: [ChatMessage] = []
    var activeIndicator: IndicatorType?
    var capturePermissionMissing = false
    var toolExecutions: [ToolExecution] = []
    var stateType: BobeStateType = .loading
}

func deriveStateType(from context: BobeContext) -> BobeStateType {
    if context.shuttingDown { return .shuttingDown }
    if context.daemonError { return .error }
    if !context.daemonConnected { return .loading }
    if context.speaking { return .speaking }
    if context.thinking { return .thinking }
    if context.lastMessage != nil, !context.speaking { return .wantsToSpeak }
    if context.captureInProgress { return .capturing }
    return .idle
}

// MARK: - Constants

enum DaemonConfig {
    static let host = "127.0.0.1"
    static let port = 8766
    static let baseURL = "http://\(host):\(port)"
}

enum WindowSizes {
    static let widthCollapsed: CGFloat = 184
    static let widthExpanded: CGFloat = 540
    static let heightCollapsed: CGFloat = 196
    static let heightAvatar: CGFloat = 180
    static let heightInput: CGFloat = 70
    static let heightExpandedChrome: CGFloat = 56
    static let heightChatViewportMin: CGFloat = 120
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
}

let inactivityTimeoutSeconds: TimeInterval = 10 * 60
let inactivityCheckIntervalSeconds: TimeInterval = 30

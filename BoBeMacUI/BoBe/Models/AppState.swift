import Foundation

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

/// Tool dispatch surfaces only as `tool_call_*` SSE events, not as an indicator.
enum IndicatorType: String, Codable, Sendable, Equatable {
    case idle = "IDLE"
    case screenCapture = "SCREEN_CAPTURE"
    case thinking = "THINKING"
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

struct FailedSendRecovery: Identifiable, Sendable {
    let id: String
    let content: String
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

enum MessageComposerBlockReason: Sendable, Equatable {
    case starting
    case reconnecting
    case thinking
    case speaking
    case capturing
    case usingTool(String)
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
    /// Recoverable trigger errors (vision breaker, capture timeout) — tertiary tint.
    var softWarning: String?
    var indicatorMessage: String?
    /// Defaults to `true` so the composer is enabled until daemon says otherwise.
    var acceptingUserMessages = true
    /// `true` during the 3s pre-clear window after `conversation_closed`.
    var conversationEnding = false
    var currentMessage = ""
    var messages: [ChatMessage] = []
    var failedSendRecoveries: [FailedSendRecovery] = []
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

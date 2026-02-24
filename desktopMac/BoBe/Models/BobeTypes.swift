import Foundation

// MARK: - Core State Types

/// UI state type derived from daemon connection + activity flags.
/// Priority order: loading > speaking > thinking > wants_to_speak > capturing > idle
enum BobeStateType: String, Sendable {
    case loading
    case error
    case idle
    case capturing
    case thinking
    case speaking
    case wantsToSpeak = "wants_to_speak"
}

/// Indicator types from daemon SSE events — matches Rust SCREAMING_SNAKE_CASE
enum IndicatorType: String, Codable, Sendable {
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

/// Chat message sender
enum MessageSender: String, Sendable {
    case user
    case bobe
}

/// A single chat message in the stacking bubble system
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

/// Tool execution tracking for indicator display
struct ToolExecution: Identifiable, Sendable {
    var id: String { toolCallId }
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

/// Full application state context
struct BobeContext: Sendable {
    var daemonConnected: Bool = false
    var daemonError: Bool = false
    var capturing: Bool = false
    var thinking: Bool = false
    var speaking: Bool = false
    var lastMessage: String?
    var currentMessage: String = ""
    var currentMessageId: String?
    var messages: [ChatMessage] = []
    var activeIndicator: IndicatorType?
    var toolExecutions: [ToolExecution] = []
    var stateType: BobeStateType = .loading
}

/// Derive UI state type from context flags
func deriveStateType(from context: BobeContext) -> BobeStateType {
    if context.daemonError { return .error }
    if !context.daemonConnected { return .loading }
    if context.speaking { return .speaking }
    if context.thinking { return .thinking }
    if context.lastMessage != nil && !context.speaking { return .wantsToSpeak }
    if context.capturing { return .capturing }
    return .idle
}

// MARK: - Constants

enum DaemonConfig {
    static let host = "127.0.0.1"
    static let port = 8766
    static let baseURL = "http://\(host):\(port)"
}

enum WindowSizes {
    static let widthCollapsed: CGFloat = 148
    static let widthExpanded: CGFloat = 340
    static let heightCollapsed: CGFloat = 180
    static let heightAvatar: CGFloat = 180
    static let heightInput: CGFloat = 70
    static let heightMessage: CGFloat = 110
    static let heightMax: CGFloat = 700
    static let margin: CGFloat = 16
}

enum SpringConfig {
    static let damping: Double = 20
    static let stiffness: Double = 300
    static let mass: Double = 0.8
}

enum IndicatorTiming {
    static let delayBeforeShow: TimeInterval = 0.3
    static let minDisplayTime: TimeInterval = 0.6
    static let thinkingMinTime: TimeInterval = 0.8
    static let toolCompleteLinger: TimeInterval = 1.5
    static let expandAnimation: TimeInterval = 0.2
}

let maxVisibleMessages = 4
let inactivityTimeoutSeconds: TimeInterval = 10 * 60
let inactivityCheckIntervalSeconds: TimeInterval = 30

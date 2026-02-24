import Foundation

// MARK: - Goals

enum GoalStatus: String, Codable, Sendable, CaseIterable {
    case active, completed, archived, unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = GoalStatus(rawValue: raw) ?? .unknown
    }
}

enum GoalPriority: String, Codable, Sendable, CaseIterable {
    case high, medium, low, unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = GoalPriority(rawValue: raw) ?? .unknown
    }
}

enum GoalSource: String, Codable, Sendable {
    case user, inferred, unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = GoalSource(rawValue: raw) ?? .unknown
    }
}

struct Goal: Identifiable, Codable, Sendable {
    let id: String
    var content: String
    var status: GoalStatus
    var priority: GoalPriority
    var source: GoalSource
    var enabled: Bool
    let createdAt: String
    var updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, content, status, priority, source, enabled
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct GoalListResponse: Codable, Sendable {
    let goals: [Goal]
    let count: Int
    let activeCount: Int

    enum CodingKeys: String, CodingKey {
        case goals, count
        case activeCount = "active_count"
    }
}

struct GoalCreateRequest: Codable, Sendable {
    let content: String
    var priority: GoalPriority?
    var enabled: Bool?
}

struct GoalUpdateRequest: Codable, Sendable {
    var content: String?
    var status: GoalStatus?
    var priority: GoalPriority?
    var enabled: Bool?
}

struct GoalActionResponse: Codable, Sendable {
    let id: String
    let status: String
    let message: String
}

// MARK: - Goal Plans

enum GoalPlanStatus: String, Codable, Sendable {
    case pendingApproval = "pending_approval"
    case approved
    case autoApproved = "auto_approved"
    case inProgress = "in_progress"
    case completed, failed, rejected, unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = GoalPlanStatus(rawValue: raw) ?? .unknown
    }
}

enum GoalPlanStepStatus: String, Codable, Sendable {
    case pending, inProgress = "in_progress", completed, failed, skipped, unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = GoalPlanStepStatus(rawValue: raw) ?? .unknown
    }
}

struct GoalPlan: Identifiable, Codable, Sendable {
    let id: String
    let goalId: String
    let summary: String
    var status: GoalPlanStatus
    let failureCount: Int
    let lastError: String?
    let createdAt: String
    var updatedAt: String
    var steps: [GoalPlanStep]?

    enum CodingKeys: String, CodingKey {
        case id, summary, status, steps
        case goalId = "goal_id"
        case failureCount = "failure_count"
        case lastError = "last_error"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct GoalPlanStep: Identifiable, Codable, Sendable {
    let id: String
    let planId: String
    let stepOrder: Int
    let content: String
    var status: GoalPlanStepStatus
    let result: String?
    let error: String?
    let startedAt: String?
    let completedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, content, status, result, error
        case planId = "plan_id"
        case stepOrder = "step_order"
        case startedAt = "started_at"
        case completedAt = "completed_at"
    }
}

// MARK: - Souls

struct Soul: Identifiable, Codable, Sendable {
    let id: String
    var name: String
    var content: String
    var enabled: Bool
    let isDefault: Bool
    let createdAt: String
    var updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, name, content, enabled
        case isDefault = "is_default"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct SoulListResponse: Codable, Sendable {
    let souls: [Soul]
    let count: Int
    let enabledCount: Int

    enum CodingKeys: String, CodingKey {
        case souls, count
        case enabledCount = "enabled_count"
    }
}

struct SoulCreateRequest: Codable, Sendable {
    let name: String
    let content: String
    var enabled: Bool?
}

struct SoulUpdateRequest: Codable, Sendable {
    var content: String?
    var enabled: Bool?
}

struct SoulActionResponse: Codable, Sendable {
    let id: String
    let name: String
    let enabled: Bool
    let message: String
}

// MARK: - User Profiles

struct UserProfile: Identifiable, Codable, Sendable {
    let id: String
    var name: String
    var content: String
    var enabled: Bool
    let isDefault: Bool
    let createdAt: String
    var updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, name, content, enabled
        case isDefault = "is_default"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct UserProfileListResponse: Codable, Sendable {
    let profiles: [UserProfile]
    let count: Int
    let enabledCount: Int

    enum CodingKeys: String, CodingKey {
        case profiles, count
        case enabledCount = "enabled_count"
    }
}

struct UserProfileCreateRequest: Codable, Sendable {
    let name: String
    let content: String
    var enabled: Bool?
}

struct UserProfileUpdateRequest: Codable, Sendable {
    var content: String?
    var enabled: Bool?
}

struct UserProfileActionResponse: Codable, Sendable {
    let id: String
    let name: String
    let enabled: Bool
    let message: String
}

// MARK: - Memories

enum MemoryType: String, Codable, Sendable {
    case shortTerm = "short_term"
    case longTerm = "long_term"
    case explicit
    case unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = MemoryType(rawValue: raw) ?? .unknown
    }
}

enum MemoryCategory: String, Codable, Sendable, CaseIterable {
    case preference, pattern, fact, interest, general, observation, unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = MemoryCategory(rawValue: raw) ?? .unknown
    }
}

enum MemorySource: String, Codable, Sendable {
    case observation, conversation, user
    case visualDiary = "visual_diary"
    case unknown

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = MemorySource(rawValue: raw) ?? .unknown
    }
}

struct Memory: Identifiable, Codable, Sendable {
    let id: String
    var content: String
    var memoryType: MemoryType
    var category: MemoryCategory
    let source: MemorySource
    var enabled: Bool
    let createdAt: String
    var updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, content, category, source, enabled
        case memoryType = "memory_type"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct MemoryListResponse: Codable, Sendable {
    let memories: [Memory]
    let count: Int
    let total: Int
}

/// Query parameters for GET /memories
struct MemoryListParams: Sendable {
    var memoryType: MemoryType?
    var category: MemoryCategory?
    var source: MemorySource?
    var enabledOnly: Bool?
    var limit: Int?
    var offset: Int?

    var queryItems: [URLQueryItem] {
        var items: [URLQueryItem] = []
        if let v = memoryType { items.append(.init(name: "memory_type", value: v.rawValue)) }
        if let v = category { items.append(.init(name: "category", value: v.rawValue)) }
        if let v = source { items.append(.init(name: "source", value: v.rawValue)) }
        if let v = enabledOnly { items.append(.init(name: "enabled_only", value: v ? "true" : "false")) }
        if let v = limit { items.append(.init(name: "limit", value: String(v))) }
        if let v = offset { items.append(.init(name: "offset", value: String(v))) }
        return items
    }
}

struct MemoryCreateRequest: Codable, Sendable {
    let content: String
    var category: MemoryCategory?
    var memoryType: MemoryType?

    enum CodingKeys: String, CodingKey {
        case content, category
        case memoryType = "memory_type"
    }
}

struct MemoryUpdateRequest: Codable, Sendable {
    var content: String?
    var category: MemoryCategory?
    var enabled: Bool?
}

struct MemoryActionResponse: Codable, Sendable {
    let id: String
    let enabled: Bool
    let message: String
}

// MARK: - Tools

struct ToolInfo: Identifiable, Codable, Sendable {
    var id: String { name }
    let name: String
    let description: String
    let provider: String
    var enabled: Bool
    let category: String?
}

struct ToolListResponse: Codable, Sendable {
    let tools: [ToolInfo]
    let count: Int
    let providers: [String]
}

struct ToolUpdateResponse: Codable, Sendable {
    let name: String
    let enabled: Bool
    let message: String
}

// MARK: - MCP Servers

struct MCPServer: Identifiable, Codable, Sendable {
    let id: String
    let name: String
    let command: String
    let args: [String]
    var connected: Bool
    var enabled: Bool
    var toolCount: Int
    var excludedTools: [String]
    var error: String?

    enum CodingKeys: String, CodingKey {
        case id, name, command, args, connected, enabled, error
        case toolCount = "tool_count"
        case excludedTools = "excluded_tools"
    }
}

struct MCPServerListResponse: Codable, Sendable {
    let servers: [MCPServer]
    let count: Int
    let connectedCount: Int

    enum CodingKeys: String, CodingKey {
        case servers, count
        case connectedCount = "connected_count"
    }
}

struct MCPServerCreateRequest: Codable, Sendable {
    let name: String
    let command: String
    var args: [String]?
    var env: [String: String]?
    var enabled: Bool?
    var excludedTools: [String]?

    enum CodingKeys: String, CodingKey {
        case name, command, args, env, enabled
        case excludedTools = "excluded_tools"
    }
}

struct MCPServerCreateResponse: Codable, Sendable {
    let name: String
    let connected: Bool
    let toolCount: Int
    let message: String
    let error: String?

    enum CodingKeys: String, CodingKey {
        case name, connected, message, error
        case toolCount = "tool_count"
    }
}

struct MCPServerReconnectResponse: Codable, Sendable {
    let name: String
    let connected: Bool
    let toolCount: Int
    let message: String
    let error: String?

    enum CodingKeys: String, CodingKey {
        case name, connected, message, error
        case toolCount = "tool_count"
    }
}

struct MCPServerUpdateRequest: Codable, Sendable {
    var excludedTools: [String]?

    enum CodingKeys: String, CodingKey {
        case excludedTools = "excluded_tools"
    }
}

struct MCPServerUpdateResponse: Codable, Sendable {
    let name: String
    let excludedTools: [String]
    let message: String

    enum CodingKeys: String, CodingKey {
        case name, message
        case excludedTools = "excluded_tools"
    }
}

struct MCPServerDeleteResponse: Codable, Sendable {
    let name: String
    let message: String
}

// MARK: - MCP Configs

struct MCPConfig: Identifiable, Codable, Sendable {
    let id: String
    let serverName: String
    let excludedTools: [String]

    enum CodingKeys: String, CodingKey {
        case id
        case serverName = "server_name"
        case excludedTools = "excluded_tools"
    }
}

struct MCPConfigListResponse: Codable, Sendable {
    let configs: [MCPConfig]
    let count: Int
    let enabledCount: Int

    enum CodingKeys: String, CodingKey {
        case configs, count
        case enabledCount = "enabled_count"
    }
}

// MARK: - Pull Progress (SSE)

struct PullProgressEvent: Codable, Sendable {
    let status: String
    let total: Int64?
    let completed: Int64?
}

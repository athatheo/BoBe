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
    var id: String {
        self.name
    }

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

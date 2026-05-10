import Foundation

enum DaemonError: Error, LocalizedError {
    case invalidResponse
    case httpError(statusCode: Int, message: String)
    case connectionFailed
    case operationFailed(String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse: "Invalid response from daemon"
        case let .httpError(code, msg): "HTTP \(code): \(msg)"
        case .connectionFailed: "Failed to connect to daemon"
        case let .operationFailed(message): message
        }
    }
}

struct AnyEncodable: Encodable {
    private let encode: (Encoder) throws -> Void

    init(_ wrapped: any Encodable) {
        self.encode = wrapped.encode
    }

    func encode(to encoder: Encoder) throws {
        try self.encode(encoder)
    }
}

extension DaemonClient {
    // MARK: MCP Servers

    func getMCPConfig() async throws -> MCPConfigDocumentResponse {
        try await fetch("/tools/mcp/config")
    }

    func validateMCPConfig(_ request: MCPConfigMutationRequest) async throws -> MCPConfigValidateResponse {
        try await fetch("/tools/mcp/config/validate", method: "POST", body: request)
    }

    func saveMCPConfig(_ request: MCPConfigMutationRequest) async throws -> MCPConfigSaveResponse {
        try await fetch("/tools/mcp/config", method: "PUT", body: request)
    }

    func resetMCPConfig() async throws -> MCPConfigResetResponse {
        try await fetch("/tools/mcp/config", method: "DELETE")
    }

    // MARK: Settings

    func getSettings() async throws -> DaemonSettings {
        try await fetch("/settings")
    }

    func updateSettings(_ request: SettingsUpdateRequest) async throws -> SettingsUpdateResponse {
        try await fetch("/settings", method: "PATCH", body: request)
    }

    // MARK: Status

    /// Snapshot of `accepting_user_messages` + indicator + capturing.
    /// Used to seed local state after SSE reconnect.
    func getStatus() async throws -> StatusResponse {
        try await fetch("/status")
    }

    // MARK: Engine + auth + models

    /// Wraps the SDK's `Client::get_auth_status()`. Returns whether the
    /// bundled CLI sees the user signed in to GitHub Copilot — same auth
    /// the user's `gh` / `copilot` commands see (no separate login).
    func getAuthStatus() async throws -> AuthStatusResponse {
        try await fetch("/auth/status")
    }

    /// Lists models available in the given engine. `engine == nil` falls
    /// back to the daemon's current `Config.engine`. Cloud returns
    /// subscription-gated Copilot models; local hits the user's Ollama
    /// `/api/tags` (returns 503 if Ollama isn't running).
    func listModels(engine: String? = nil) async throws -> ListModelsResponse {
        let suffix = engine.map { "?engine=\($0)" } ?? ""
        return try await fetch("/models\(suffix)")
    }
}

// MARK: - Auth + models DTOs

struct AuthStatusResponse: Codable, Sendable {
    let isAuthenticated: Bool
    let authType: String?
    let host: String?
    let login: String?
    let statusMessage: String?

    enum CodingKeys: String, CodingKey {
        case isAuthenticated = "is_authenticated"
        case authType = "auth_type"
        case host
        case login
        case statusMessage = "status_message"
    }
}

struct ListModelsResponse: Codable, Sendable {
    /// `"copilot_cloud"` or `"local"` — echoes the engine that was queried.
    let engine: String
    let models: [ModelInfo]
}

struct ModelInfo: Codable, Sendable, Identifiable, Hashable {
    /// Stable identifier — `"claude-sonnet-4.5"` for cloud,
    /// `"qwen2.5:7b-instruct"` for local Ollama tags.
    let id: String
    /// Display name — falls back to `id` when not provided.
    let name: String
    /// Whether the model accepts image inputs. Filters the Vision model
    /// dropdown to exclude text-only models.
    let vision: Bool
    /// Maximum context window in tokens, when reported. Cloud models
    /// always populate this; Ollama doesn't expose it via `/api/tags`
    /// so it'll be `nil` there.
    let contextWindow: Int?

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case vision
        case contextWindow = "context_window"
    }
}

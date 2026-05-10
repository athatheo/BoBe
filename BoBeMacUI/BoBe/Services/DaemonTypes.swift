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
    /// Absolute path to the bundled Copilot CLI binary on disk (the one
    /// the SDK extracted from BoBe's own binary). Use this to launch
    /// Terminal with the bundled CLI for sign-in — no external `gh`,
    /// no separate install. `nil` only if the bundled-CLI feature is
    /// disabled or extraction hasn't happened yet.
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

// MARK: - Local-runtime install DTOs

extension DaemonClient {
    /// Kicks off the wizard's local-mode install: ensures Ollama is
    /// running (downloading the runtime if needed), pulls each model.
    /// Returns 202 immediately; the wizard listens on
    /// `streamLocalRuntimeStatus()` for progress.
    @discardableResult
    func startLocalRuntimeInstall(_ request: LocalRuntimeInstallRequest) async throws -> LocalRuntimeMessageResponse {
        try await fetch("/local-runtime/install", method: "POST", body: request)
    }

    /// Signals the daemon to abort an in-flight install. Snapshot stream
    /// will emit `status: "canceled"` shortly after.
    @discardableResult
    func cancelLocalRuntimeInstall() async throws -> LocalRuntimeMessageResponse {
        try await fetch("/local-runtime/cancel", method: "POST")
    }

    /// Open an SSE connection to `/local-runtime/status`, call
    /// `onSnapshot` for each parsed event. Returns when a terminal
    /// status arrives (`complete` / `canceled` / `failed`) or when the
    /// caller's `Task` is cancelled.
    func streamLocalRuntimeStatus(
        onSnapshot: @Sendable @escaping (LocalRuntimeSnapshot) -> Void
    ) async throws {
        let url = self.endpointURL("local-runtime/status")
        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 0

        let (bytes, response) = try await self.session.bytes(for: request)
        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200
        else {
            throw DaemonError.invalidResponse
        }

        let decoder = JSONDecoder()
        for try await line in bytes.lines {
            if Task.isCancelled { break }
            guard line.hasPrefix("data: ") else { continue }
            let jsonStr = String(line.dropFirst(6))
            guard let data = jsonStr.data(using: .utf8) else { continue }
            do {
                let snapshot = try decoder.decode(LocalRuntimeSnapshot.self, from: data)
                onSnapshot(snapshot)
                if ["complete", "canceled", "failed"].contains(snapshot.status) {
                    break
                }
            } catch {
                continue
            }
        }
    }
}

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

/// Snapshot the daemon emits on the install-status SSE stream. UI binds
/// progress bars to `runtime.percent`, `chatModel.percent`, etc.
struct LocalRuntimeSnapshot: Codable, Sendable {
    /// `"idle"`, `"running"`, `"complete"`, `"canceled"`, or `"failed"`.
    let status: String
    /// Set when `status == "failed"`.
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
    /// Phase string from Ollama (`"pulling manifest"`, `"downloading"`,
    /// `"verifying sha256 digest"`, `"writing manifest"`, `"success"`)
    /// or our own (`"already installed"`, `"skipped (same as chat)"`).
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

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

    /// Used to seed local state after SSE reconnect.
    func getStatus() async throws -> StatusResponse {
        try await fetch("/status")
    }

    // MARK: Engine + auth + models

    func getAuthStatus() async throws -> AuthStatusResponse {
        try await fetch("/auth/status")
    }

    /// `engine == nil` uses daemon's current `Config.engine`. Local returns 503 if Ollama is down.
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
    /// `nil` if the bundled-CLI feature is disabled or not yet extracted.
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
    let engine: String
    let models: [ModelInfo]
}

struct ModelInfo: Codable, Sendable, Identifiable, Hashable {
    let id: String
    let name: String
    let vision: Bool
    /// `nil` for Ollama — `/api/tags` doesn't expose context window.
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
    /// Returns 202 immediately; listen on `streamLocalRuntimeStatus()` for progress.
    @discardableResult
    func startLocalRuntimeInstall(_ request: LocalRuntimeInstallRequest) async throws -> LocalRuntimeMessageResponse {
        try await fetch("/local-runtime/install", method: "POST", body: request)
    }

    @discardableResult
    func cancelLocalRuntimeInstall() async throws -> LocalRuntimeMessageResponse {
        try await fetch("/local-runtime/cancel", method: "POST")
    }

    /// Returns on terminal status (`complete`/`canceled`/`failed`) or task cancel.
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

struct LocalRuntimeSnapshot: Codable, Sendable {
    let status: String
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

// MARK: - Voice install DTOs

struct VoiceInstallSnapshot: Codable, Sendable {
    let status: String
    let models: [VoiceModelProgress]
    let installed: VoiceInstallPresence

    /// True when the daemon reports an active install. Matches the Rust
    /// `InstallStatus::Running` variant's wire form.
    var isRunning: Bool { self.status == "running" }
    /// Convenience for the wizard step's continue button.
    var isComplete: Bool { self.status == "complete" }
    var isTerminal: Bool { ["complete", "canceled", "failed", "idle"].contains(self.status) }
}

struct VoiceModelProgress: Codable, Sendable, Identifiable {
    let kind: String
    let label: String
    let status: String
    let bytesDownloaded: UInt64
    let bytesTotal: UInt64?
    let percent: Int?

    var id: String { self.kind }

    enum CodingKeys: String, CodingKey {
        case kind
        case label
        case status
        case bytesDownloaded = "bytes_downloaded"
        case bytesTotal = "bytes_total"
        case percent
    }
}

struct VoiceInstallPresence: Codable, Sendable {
    let streamingStt: Bool
    let tts: Bool
    let vad: Bool
    let smartTurn: Bool
    let allPresent: Bool

    enum CodingKeys: String, CodingKey {
        case streamingStt = "streaming_stt"
        case tts
        case vad
        case smartTurn = "smart_turn"
        case allPresent = "all_present"
    }
}

extension DaemonClient {
    func voiceInstallStatus() async throws -> VoiceInstallSnapshot {
        try await self.fetch("/voice/install/status")
    }

    func startVoiceInstall() async throws {
        try await self.fetchVoid("/voice/install/start", method: "POST")
    }

    func cancelVoiceInstall() async throws {
        try await self.fetchVoid("/voice/install/cancel", method: "POST")
    }
}

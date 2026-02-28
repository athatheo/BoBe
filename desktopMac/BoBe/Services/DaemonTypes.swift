import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "DaemonClient")

// MARK: - Errors

enum DaemonError: Error, LocalizedError {
    case invalidResponse
    case httpError(statusCode: Int, message: String)
    case connectionFailed

    var errorDescription: String? {
        switch self {
        case .invalidResponse: "Invalid response from daemon"
        case .httpError(let code, let msg): "HTTP \(code): \(msg)"
        case .connectionFailed: "Failed to connect to daemon"
        }
    }
}

// MARK: - Type-Erased Encodable

struct AnyEncodable: Encodable {
    private let encode: (Encoder) throws -> Void

    init(_ wrapped: any Encodable) {
        self.encode = wrapped.encode
    }

    func encode(to encoder: Encoder) throws {
        try encode(encoder)
    }
}

// MARK: - DaemonClient Tools, MCP, Settings, Models & Onboarding

extension DaemonClient {

    // MARK: Tools

    func listTools() async throws -> ToolListResponse {
        try await fetch("/tools")
    }

    func enableTool(_ name: String) async throws -> ToolUpdateResponse {
        try await fetch("/tools/\(name)/enable", method: "POST")
    }

    func disableTool(_ name: String) async throws -> ToolUpdateResponse {
        try await fetch("/tools/\(name)/disable", method: "POST")
    }

    // MARK: MCP Servers

    func listMCPServers() async throws -> MCPServerListResponse {
        try await fetch("/tools/mcp")
    }

    func createMCPServer(_ request: MCPServerCreateRequest) async throws -> MCPServerCreateResponse {
        try await fetch("/tools/mcp", method: "POST", body: request)
    }

    func deleteMCPServer(_ name: String) async throws {
        try await fetchVoid("/tools/mcp/\(name)", method: "DELETE")
    }

    func reconnectMCPServer(_ name: String) async throws -> MCPServerReconnectResponse {
        try await fetch("/tools/mcp/\(name)/reconnect", method: "POST")
    }

    func updateMCPServer(_ name: String, _ request: MCPServerUpdateRequest) async throws -> MCPServerUpdateResponse {
        try await fetch("/tools/mcp/\(name)", method: "PATCH", body: request)
    }

    // MARK: Goal Worker

    func goalWorkerStatus() async throws -> GoalWorkerStatusResponse {
        try await fetch("/goal-plans/status")
    }

    // MARK: Settings

    func getSettings() async throws -> DaemonSettings {
        try await fetch("/settings")
    }

    func updateSettings(_ request: SettingsUpdateRequest) async throws -> SettingsUpdateResponse {
        try await fetch("/settings", method: "PATCH", body: request)
    }

    // MARK: Models

    func listModels() async throws -> ModelsListResponse {
        try await fetch("/models")
    }

    func pullModel(_ name: String) async throws {
        try await fetchVoid("/models/pull", method: "POST", body: ["model": name])
    }

    func deleteModel(_ name: String) async throws {
        try await fetchVoid("/models/\(name)", method: "DELETE")
    }

    // MARK: Onboarding

    func getOnboardingStatus() async throws -> OnboardingStatusResponse {
        try await fetch("/onboarding/status")
    }

    func getOnboardingOptions() async throws -> OnboardingOptions {
        try await fetch("/onboarding/options")
    }

    func startSetupJob(_ request: SetupRequest) async throws -> SetupJobState {
        try await fetch("/onboarding/setup", method: "POST", body: request)
    }

    func getSetupJobStatus(jobId: String) async throws -> SetupJobState {
        try await fetch("/onboarding/setup/\(jobId)")
    }

    func cancelSetupJob(jobId: String) async throws -> SetupJobState {
        try await fetch("/onboarding/setup/\(jobId)", method: "DELETE")
    }

    func markOnboardingComplete() async throws {
        try await fetchVoid("/onboarding/mark-complete", method: "POST")
    }

    /// Warmup the embedding model (2 minute timeout)
    func warmupEmbedding() async throws {
        let url = baseURL.appendingPathComponent("onboarding/warmup-embedding")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 120
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            logger.error("POST /onboarding/warmup-embedding: network error — \(error.localizedDescription)")
            throw error
        }
        guard let httpResponse = response as? HTTPURLResponse,
              (200...299).contains(httpResponse.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "Unknown error"
            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
            logger.error("POST /onboarding/warmup-embedding failed: HTTP \(code) — \(message)")
            throw DaemonError.httpError(
                statusCode: code,
                message: message)
        }
    }
}

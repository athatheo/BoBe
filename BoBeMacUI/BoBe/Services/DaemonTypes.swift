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
        try await performModelPull(named: name)
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
}

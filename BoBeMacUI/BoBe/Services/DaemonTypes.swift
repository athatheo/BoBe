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
}

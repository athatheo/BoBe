import Foundation

/// Per-domain HTTP API methods on `DaemonClient`. Each section maps 1:1 to
/// a daemon route group. DTOs live in `Models/DaemonDTOs.swift`; the SSE
/// stream loop + reconnect logic stays in `DaemonClient.swift`.
extension DaemonClient {
    // MARK: MCP servers

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

    // MARK: Local runtime install

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
                if [.complete, .canceled, .failed].contains(snapshot.status) {
                    break
                }
            } catch {
                continue
            }
        }
    }

    // MARK: Voice install

    func voiceInstallStatus() async throws -> VoiceInstallSnapshot {
        try await self.fetch("/voice/install/status")
    }

    func startVoiceInstall() async throws {
        try await self.fetchVoid("/voice/install/start", method: "POST")
    }

    func cancelVoiceInstall() async throws {
        try await self.fetchVoid("/voice/install/cancel", method: "POST")
    }

    // MARK: Copilot login (cloud-auth device flow)

    /// Kicks off the bundled CLI's device flow. Returns 202; the device
    /// code and URL stream via `streamCopilotLogin()`.
    func startCopilotLogin() async throws {
        try await self.fetchVoid("/auth/copilot/login/start", method: "POST")
    }

    func cancelCopilotLogin() async throws {
        try await self.fetchVoid("/auth/copilot/login/cancel", method: "POST")
    }

    /// Streams login phase updates. Caller-side cancellation via `Task.cancel()`
    /// is the only way to stop the stream — terminal phases (`completed`,
    /// `failed`, `canceled`) close the loop, but the daemon's watch channel
    /// holds the last value so a fresh `subscribe()` always sees the
    /// terminal phase.
    func streamCopilotLogin(
        onPhase: @Sendable @escaping (CopilotLoginPhase) -> Void
    ) async throws {
        let url = self.endpointURL("auth/copilot/login/events")
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
                let phase = try decoder.decode(CopilotLoginPhase.self, from: data)
                onPhase(phase)
                if phase.isTerminal {
                    break
                }
            } catch {
                continue
            }
        }
    }
}

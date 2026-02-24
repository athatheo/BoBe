import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "DaemonClient")

/// HTTP + SSE client for communicating with the Rust backend daemon
actor DaemonClient {
    static let shared = DaemonClient()

    private let baseURL = URL(string: DaemonConfig.baseURL)!
    private let session: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    private let fetchTimeout: TimeInterval = 10

    private var sseTask: Task<Void, Never>?
    private var eventHandler: ((StreamBundle) -> Void)?
    private var connectionHandler: ((Bool) -> Void)?
    private var reconnectAttempts = 0
    private let maxReconnectAttempts = 10

    init() {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 300
        self.session = URLSession(configuration: config)
        self.decoder = JSONDecoder()
        self.encoder = JSONEncoder()
    }

    // MARK: - SSE Connection

    func connectSSE(
        onEvent: @escaping @Sendable (StreamBundle) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) {
        self.eventHandler = onEvent
        self.connectionHandler = onConnectionChange
        reconnectAttempts = 0
        startSSE()
    }

    func disconnectSSE() {
        sseTask?.cancel()
        sseTask = nil
        eventHandler = nil
        connectionHandler = nil
    }

    private func startSSE() {
        sseTask?.cancel()
        sseTask = Task { [weak self] in
            guard let self else { return }
            await self.runSSELoop()
        }
    }

    private func runSSELoop() async {
        let url = baseURL.appendingPathComponent("events")
        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 0 // No timeout for SSE

        do {
            let (bytes, response) = try await session.bytes(for: request)
            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200 else {
                logger.warning("SSE connection failed with non-200 status")
                await handleSSEDisconnect()
                return
            }

            logger.info("SSE connected")
            reconnectAttempts = 0
            connectionHandler?(true)

            for try await line in bytes.lines {
                if Task.isCancelled { break }
                guard line.hasPrefix("data: ") else { continue }
                let jsonStr = String(line.dropFirst(6))
                guard let data = jsonStr.data(using: .utf8) else { continue }
                do {
                    let bundle = try decoder.decode(StreamBundle.self, from: data)
                    eventHandler?(bundle)
                } catch {
                    logger.error("Failed to decode SSE event: \(error.localizedDescription)")
                }
            }
        } catch {
            if !Task.isCancelled {
                logger.warning("SSE stream error: \(error.localizedDescription)")
            }
        }

        if !Task.isCancelled {
            await handleSSEDisconnect()
        }
    }

    private func handleSSEDisconnect() async {
        connectionHandler?(false)
        reconnectAttempts += 1
        guard reconnectAttempts <= maxReconnectAttempts else {
            logger.error("Max SSE reconnect attempts reached")
            return
        }
        // Exponential backoff: 1s, 2s, 4s, 8s... capped at 30s
        let delay = min(pow(2.0, Double(reconnectAttempts - 1)), 30.0)
        logger.info("SSE reconnecting in \(delay)s (attempt \(self.reconnectAttempts))")
        try? await Task.sleep(for: .seconds(delay))
        if !Task.isCancelled {
            startSSE()
        }
    }

    // MARK: - HTTP Helpers

    private func fetch<T: Decodable>(
        _ path: String,
        method: String = "GET",
        body: (any Encodable)? = nil
    ) async throws -> T {
        let url = baseURL.appendingPathComponent(path)
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = fetchTimeout

        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try encoder.encode(AnyEncodable(body))
        }

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw DaemonError.invalidResponse
        }
        guard (200...299).contains(httpResponse.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw DaemonError.httpError(statusCode: httpResponse.statusCode, message: message)
        }
        return try decoder.decode(T.self, from: data)
    }

    private func fetchVoid(
        _ path: String,
        method: String = "POST",
        body: (any Encodable)? = nil
    ) async throws {
        let url = baseURL.appendingPathComponent(path)
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = fetchTimeout

        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try encoder.encode(AnyEncodable(body))
        }

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw DaemonError.invalidResponse
        }
        guard (200...299).contains(httpResponse.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw DaemonError.httpError(statusCode: httpResponse.statusCode, message: message)
        }
    }

    // MARK: - Health & Status

    func health() async throws -> HealthResponse {
        try await fetch("/health")
    }

    func status() async throws -> [String: AnyCodableValue] {
        try await fetch("/status")
    }

    // MARK: - Capture

    func startCapture() async throws {
        try await fetchVoid("/capture/start")
    }

    func stopCapture() async throws {
        try await fetchVoid("/capture/stop")
    }

    // MARK: - Messages

    func sendMessage(_ content: String) async throws -> SendMessageResponse {
        try await fetch("/message", method: "POST", body: SendMessageRequest(content: content))
    }

    func dismissMessage() async throws {
        try await fetchVoid("/message/dismiss")
    }

    // MARK: - Goals

    func listGoals() async throws -> GoalListResponse {
        try await fetch("/goals")
    }

    func getGoal(_ id: String) async throws -> Goal {
        try await fetch("/goals/\(id)")
    }

    func createGoal(_ request: GoalCreateRequest) async throws -> Goal {
        try await fetch("/goals", method: "POST", body: request)
    }

    func updateGoal(_ id: String, _ request: GoalUpdateRequest) async throws -> Goal {
        try await fetch("/goals/\(id)", method: "PATCH", body: request)
    }

    func deleteGoal(_ id: String) async throws -> GoalActionResponse {
        try await fetch("/goals/\(id)", method: "DELETE")
    }

    func completeGoal(_ id: String) async throws -> GoalActionResponse {
        try await fetch("/goals/\(id)/complete", method: "POST")
    }

    func archiveGoal(_ id: String) async throws -> GoalActionResponse {
        try await fetch("/goals/\(id)/archive", method: "POST")
    }

    // MARK: - Souls

    func listSouls() async throws -> SoulListResponse {
        try await fetch("/souls")
    }

    func getSoul(_ id: String) async throws -> Soul {
        try await fetch("/souls/\(id)")
    }

    func createSoul(_ request: SoulCreateRequest) async throws -> Soul {
        try await fetch("/souls", method: "POST", body: request)
    }

    func updateSoul(_ id: String, _ request: SoulUpdateRequest) async throws -> Soul {
        try await fetch("/souls/\(id)", method: "PATCH", body: request)
    }

    func deleteSoul(_ id: String) async throws -> SoulActionResponse {
        try await fetch("/souls/\(id)", method: "DELETE")
    }

    func enableSoul(_ id: String) async throws -> SoulActionResponse {
        try await fetch("/souls/\(id)/enable", method: "POST")
    }

    func disableSoul(_ id: String) async throws -> SoulActionResponse {
        try await fetch("/souls/\(id)/disable", method: "POST")
    }

    // MARK: - User Profiles

    func listUserProfiles() async throws -> UserProfileListResponse {
        try await fetch("/user-profiles")
    }

    func createUserProfile(_ request: UserProfileCreateRequest) async throws -> UserProfile {
        try await fetch("/user-profiles", method: "POST", body: request)
    }

    func updateUserProfile(_ id: String, _ request: UserProfileUpdateRequest) async throws -> UserProfile {
        try await fetch("/user-profiles/\(id)", method: "PATCH", body: request)
    }

    func deleteUserProfile(_ id: String) async throws -> UserProfileActionResponse {
        try await fetch("/user-profiles/\(id)", method: "DELETE")
    }

    func enableUserProfile(_ id: String) async throws -> UserProfileActionResponse {
        try await fetch("/user-profiles/\(id)/enable", method: "POST")
    }

    func disableUserProfile(_ id: String) async throws -> UserProfileActionResponse {
        try await fetch("/user-profiles/\(id)/disable", method: "POST")
    }

    // MARK: - Memories

    func listMemories(
        type: MemoryType? = nil,
        category: MemoryCategory? = nil,
        limit: Int? = nil,
        offset: Int? = nil
    ) async throws -> MemoryListResponse {
        var components = URLComponents(url: baseURL.appendingPathComponent("/memories"), resolvingAgainstBaseURL: false)!
        var items: [URLQueryItem] = []
        if let type { items.append(.init(name: "memory_type", value: type.rawValue)) }
        if let category { items.append(.init(name: "category", value: category.rawValue)) }
        if let limit { items.append(.init(name: "limit", value: String(limit))) }
        if let offset { items.append(.init(name: "offset", value: String(offset))) }
        if !items.isEmpty { components.queryItems = items }

        var request = URLRequest(url: components.url!)
        request.timeoutInterval = fetchTimeout
        let (data, _) = try await session.data(for: request)
        return try decoder.decode(MemoryListResponse.self, from: data)
    }

    func createMemory(_ request: MemoryCreateRequest) async throws -> Memory {
        try await fetch("/memories", method: "POST", body: request)
    }

    func updateMemory(_ id: String, _ request: MemoryUpdateRequest) async throws -> Memory {
        try await fetch("/memories/\(id)", method: "PATCH", body: request)
    }

    func deleteMemory(_ id: String) async throws -> MemoryActionResponse {
        try await fetch("/memories/\(id)", method: "DELETE")
    }

    func enableMemory(_ id: String) async throws -> MemoryActionResponse {
        try await fetch("/memories/\(id)/enable", method: "POST")
    }

    func disableMemory(_ id: String) async throws -> MemoryActionResponse {
        try await fetch("/memories/\(id)/disable", method: "POST")
    }

    // MARK: - Tools

    func listTools() async throws -> ToolListResponse {
        try await fetch("/tools")
    }

    func enableTool(_ name: String) async throws -> ToolUpdateResponse {
        try await fetch("/tools/\(name)/enable", method: "POST")
    }

    func disableTool(_ name: String) async throws -> ToolUpdateResponse {
        try await fetch("/tools/\(name)/disable", method: "POST")
    }

    // MARK: - MCP Servers

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

    // MARK: - Settings

    func getSettings() async throws -> DaemonSettings {
        try await fetch("/settings")
    }

    func updateSettings(_ request: SettingsUpdateRequest) async throws -> SettingsUpdateResponse {
        try await fetch("/settings", method: "PATCH", body: request)
    }

    // MARK: - Models

    func listModels() async throws -> ModelsListResponse {
        try await fetch("/models")
    }

    func pullModel(_ name: String) async throws -> ModelActionResponse {
        try await fetch("/models/pull", method: "POST", body: ["name": name])
    }

    func deleteModel(_ name: String) async throws -> ModelActionResponse {
        try await fetch("/models/\(name)", method: "DELETE")
    }

    // MARK: - Onboarding

    func getOnboardingStatus() async throws -> OnboardingStatusResponse {
        try await fetch("/onboarding/status")
    }

    func configureLLM(_ request: ConfigureLLMRequest) async throws {
        try await fetchVoid("/onboarding/configure-llm", method: "POST", body: request)
    }

    func markOnboardingComplete() async throws {
        try await fetchVoid("/onboarding/mark-complete", method: "POST")
    }

    /// Stream model pull progress via SSE — returns (status, percent) updates
    /// Uses /models/pull which is the SSE streaming endpoint
    func pullModelSSE(model: String, onProgress: @Sendable @escaping (String, Double) -> Void) async throws {
        let url = baseURL.appendingPathComponent("models/pull")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 600
        let body = ["name": model]
        request.httpBody = try JSONEncoder().encode(body)

        let (bytes, response) = try await session.bytes(for: request)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            throw DaemonError.invalidResponse
        }

        for try await line in bytes.lines {
            if line.hasPrefix("data: ") {
                let json = String(line.dropFirst(6))
                if let data = json.data(using: .utf8),
                   let event = try? JSONDecoder().decode(PullProgressEvent.self, from: data) {
                    if event.status == "error" {
                        throw DaemonError.httpError(statusCode: 500, message: "Model pull failed")
                    }
                    let total = event.total ?? 0
                    let completed = event.completed ?? 0
                    let percent = total > 0 ? Double(completed) / Double(total) * 100.0 : 0
                    onProgress(event.status, percent)
                    if event.status == "success" || event.status == "complete" { return }
                }
            }
        }
    }

    /// Warmup the embedding model (2 minute timeout — first download can be slow)
    func warmupEmbedding() async throws {
        let url = baseURL.appendingPathComponent("onboarding/warmup-embedding")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 120
        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse,
              (200...299).contains(httpResponse.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw DaemonError.httpError(statusCode: (response as? HTTPURLResponse)?.statusCode ?? 0, message: message)
        }
    }

    /// Download a voice model (STT or TTS) via SSE progress stream.
    /// NOTE: Voice endpoints are not yet implemented in the Rust backend (STT/TTS excluded from migration).
    /// This is a placeholder that will work once /voice/models/download is added.
    func downloadVoiceModel(
        backend: String,
        modelName: String,
        onProgress: @Sendable @escaping (String, Double) -> Void
    ) async throws {
        let url = baseURL.appendingPathComponent("voice/models/download")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 300
        let body: [String: String] = ["backend": backend, "model_name": modelName]
        request.httpBody = try JSONEncoder().encode(body)

        let (bytes, response) = try await session.bytes(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw DaemonError.httpError(statusCode: 0, message: "Voice model download not yet available — voice endpoints are not implemented in the backend")
        }
        guard httpResponse.statusCode == 200 else {
            if httpResponse.statusCode == 404 {
                throw DaemonError.httpError(statusCode: 404, message: "Voice model download not available — backend does not support voice features yet")
            }
            throw DaemonError.invalidResponse
        }

        for try await line in bytes.lines {
            if line.hasPrefix("data: ") {
                let json = String(line.dropFirst(6))
                if let data = json.data(using: .utf8),
                   let event = try? JSONDecoder().decode(PullProgressEvent.self, from: data) {
                    if event.status == "error" {
                        throw DaemonError.httpError(statusCode: 500, message: "Voice model download failed")
                    }
                    let total = event.total ?? 0
                    let completed = event.completed ?? 0
                    let percent = total > 0 ? Double(completed) / Double(total) * 100.0 : 0
                    onProgress(event.status, percent)
                    if event.status == "complete" || event.status == "success" { return }
                }
            }
        }
    }
}

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

private struct AnyEncodable: Encodable {
    private let encode: (Encoder) throws -> Void

    init(_ wrapped: any Encodable) {
        self.encode = wrapped.encode
    }

    func encode(to encoder: Encoder) throws {
        try encode(encoder)
    }
}

import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "DaemonClient")

private struct DaemonErrorEnvelope: Decodable {
    let error: DaemonErrorBody
}

private struct DaemonErrorBody: Decodable {
    let message: String
    /// Stable machine-readable code from the daemon (e.g. `AUTH_REQUIRED`,
    /// `NO_ENTITLEMENTS`). Optional because most error variants don't supply one.
    let code: String?
}

actor DaemonClient {
    static let shared = DaemonClient()

    let baseURL = DaemonConfig.endpoint.baseURL

    let session: URLSession
    /// Dedicated session for the SSE long-poll. Keeping it separate from
    /// `session` lets SSE inherit URLSession's default 7-day
    /// `timeoutIntervalForResource` while normal HTTP keeps a 300s cap.
    /// Without this split, the shared 300s resource cap killed the SSE
    /// stream every 5 minutes — causing periodic `connection_manager.replacing_connection`
    /// churn on the daemon and a visible "Reconnecting…" flicker.
    private let sseSession: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    let fetchTimeout: TimeInterval = 10

    private var sseTask: Task<Void, Never>?
    private var eventHandler: (@Sendable (StreamBundle) async -> Void)?
    private var connectionHandler: (@Sendable (Bool) async -> Void)?
    private var reconnectAttempts = 0
    private var isReconnecting = false

    func endpointURL(_ path: String, queryItems: [URLQueryItem] = []) -> URL {
        let trimmed = path.hasPrefix("/") ? String(path.dropFirst()) : path
        let url = self.baseURL.appendingPathComponent(trimmed)
        return queryItems.isEmpty ? url : url.appending(queryItems: queryItems)
    }

    init() {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 300
        self.session = URLSession(configuration: config)

        // SSE config: keep request idle timeout generous (heartbeats arrive
        // every 15s from the daemon), and let `timeoutIntervalForResource`
        // stay at the URLSession default of 7 days so the long-poll isn't
        // forcibly torn down mid-session.
        let sseConfig = URLSessionConfiguration.default
        sseConfig.timeoutIntervalForRequest = 60
        sseConfig.shouldUseExtendedBackgroundIdleMode = true
        self.sseSession = URLSession(configuration: sseConfig)

        self.decoder = JSONDecoder()
        self.encoder = JSONEncoder()
    }

    // MARK: - SSE Connection

    func connectSSE(
        onEvent: @escaping @Sendable (StreamBundle) async -> Void,
        onConnectionChange: @escaping @Sendable (Bool) async -> Void
    ) {
        self.eventHandler = onEvent
        self.connectionHandler = onConnectionChange
        self.reconnectAttempts = 0
        self.startSSE()
    }

    func disconnectSSE() {
        self.sseTask?.cancel()
        self.sseTask = nil
        self.eventHandler = nil
        self.connectionHandler = nil
    }

    private func startSSE() {
        self.sseTask?.cancel()
        self.sseTask = Task { [weak self] in
            guard let self else { return }
            await self.runSSELoop()
        }
    }

    private func runSSELoop() async {
        let url = self.endpointURL("events")
        var request = URLRequest(url: url)
        DaemonConfig.endpoint.authorize(&request)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        // Set explicitly large rather than 0 (which inherits the session
        // default request timeout). The session default of 60s would be
        // fine on its own thanks to heartbeats, but a large explicit value
        // documents intent and is robust to future session-config changes.
        request.timeoutInterval = TimeInterval(7 * 24 * 60 * 60)

        do {
            let (bytes, response) = try await sseSession.bytes(for: request)
            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200
            else {
                logger.warning("SSE connection failed with non-200 status")
                await self.handleSSEDisconnect()
                return
            }

            logger.info("SSE connected")
            self.reconnectAttempts = 0
            await self.connectionHandler?(true)

            for try await line in bytes.lines {
                if Task.isCancelled { break }
                guard line.hasPrefix("data: ") else { continue }
                let jsonStr = String(line.dropFirst(6))
                guard let data = jsonStr.data(using: .utf8) else { continue }
                do {
                    let bundle = try decoder.decode(StreamBundle.self, from: data)
                    await self.eventHandler?(bundle)
                } catch {
                    logger.error("Failed to decode SSE event: \(error.localizedDescription, privacy: .public)")
                }
            }
        } catch {
            if !Task.isCancelled {
                logger.warning("SSE stream error: \(error.localizedDescription, privacy: .public)")
            }
        }

        if !Task.isCancelled {
            await self.handleSSEDisconnect()
        }
    }

    private func handleSSEDisconnect() async {
        guard !self.isReconnecting else { return }
        self.isReconnecting = true
        defer { isReconnecting = false }

        await self.connectionHandler?(false)
        self.reconnectAttempts += 1

        // First attempt is immediate so transient drops are invisible.
        // Subsequent attempts back off 0.5s, 1s, 2s, 4s, … capped at 15s.
        // No hard cap on attempt count — daemon may come back hours later.
        let delay: Double = if self.reconnectAttempts == 1 {
            0
        } else {
            min(pow(2.0, Double(self.reconnectAttempts - 2)) * 0.5, 15.0)
        }
        if delay > 0 {
            logger.info("SSE reconnecting in \(delay, privacy: .public)s (attempt \(self.reconnectAttempts))")
            try? await Task.sleep(for: .seconds(delay))
        }
        if !Task.isCancelled {
            self.startSSE()
        }
    }

    // MARK: - HTTP Helpers

    /// Send the request, validate `2xx`, return raw bytes. Single point
    /// of HTTP plumbing (URL build, method, content-type, body, network
    /// error, status-code check) — `fetch<T>` and `fetchVoid` are thin
    /// wrappers that decide whether to decode the body.
    private func send(
        _ path: String,
        method: String,
        body: (any Encodable)?,
        queryItems: [URLQueryItem],
        requestTimeout: TimeInterval?
    ) async throws -> Data {
        let url = self.endpointURL(path, queryItems: queryItems)
        var request = URLRequest(url: url)
        DaemonConfig.endpoint.authorize(&request)
        request.httpMethod = method
        request.timeoutInterval = requestTimeout ?? self.fetchTimeout

        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try self.encoder.encode(AnyEncodable(body))
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await self.session.data(for: request)
        } catch {
            logger.error("\(method, privacy: .public) \(path, privacy: .public): network error — \(error.localizedDescription, privacy: .public)")
            throw error
        }
        guard let httpResponse = response as? HTTPURLResponse else {
            logger.error("\(method, privacy: .public) \(path, privacy: .public): invalid response (not HTTP)")
            throw DaemonError.invalidResponse
        }
        guard (200 ... 299).contains(httpResponse.statusCode) else {
            let (message, code) = self.errorMessage(from: data)
            let codeLabel = code ?? "no-code"
            let statusCode = httpResponse.statusCode
            logger.error(
                // swiftlint:disable:next line_length
                "\(method, privacy: .public) \(path, privacy: .public) failed: HTTP \(statusCode) — \(message, privacy: .public) [\(codeLabel, privacy: .public)]"
            )
            throw DaemonError.httpError(statusCode: statusCode, message: message, code: code)
        }
        return data
    }

    func fetch<T: Decodable>(
        _ path: String,
        method: String = "GET",
        body: (any Encodable)? = nil,
        queryItems: [URLQueryItem] = [],
        requestTimeout: TimeInterval? = nil
    ) async throws -> T {
        let data = try await self.send(
            path,
            method: method,
            body: body,
            queryItems: queryItems,
            requestTimeout: requestTimeout
        )
        do {
            return try self.decoder.decode(T.self, from: data)
        } catch {
            logger.error("\(method, privacy: .public) \(path, privacy: .public): decode error — \(error.localizedDescription, privacy: .public)")
            throw error
        }
    }

    func fetchVoid(
        _ path: String,
        method: String = "POST",
        body: (any Encodable)? = nil,
        queryItems: [URLQueryItem] = [],
        requestTimeout: TimeInterval? = nil
    ) async throws {
        _ = try await self.send(
            path,
            method: method,
            body: body,
            queryItems: queryItems,
            requestTimeout: requestTimeout
        )
    }

    private func errorMessage(from data: Data) -> (message: String, code: String?) {
        if let envelope = try? self.decoder.decode(DaemonErrorEnvelope.self, from: data) {
            return (envelope.error.message, envelope.error.code)
        }
        return (String(data: data, encoding: .utf8) ?? "Unknown error", nil)
    }

    // MARK: - Health & Status

    func health() async throws -> HealthResponse {
        try await self.fetch("/health")
    }

    // MARK: - Capture

    func startCapture() async throws {
        try await self.fetchVoid("/capture/start")
    }

    func stopCapture() async throws {
        try await self.fetchVoid("/capture/stop")
    }

    // MARK: - Messages

    /// `/message` returns the stable message ID plus replay status; new output
    /// streams separately through SSE.
    @discardableResult
    func sendMessage(_ content: String, requestId: UUID) async throws -> SendMessageResponse {
        try await self.fetch(
            "/message",
            method: "POST",
            body: SendMessageRequest(content: content, requestId: requestId)
        )
    }

    // MARK: - Memory (single document)

    func getMemory() async throws -> MemoryResponse {
        try await self.fetch("/memory")
    }

    @discardableResult
    func updateMemory(_ content: String) async throws -> MemoryResponse {
        try await self.fetch(
            "/memory",
            method: "PUT",
            body: MemoryUpdateRequest(content: content)
        )
    }

    // MARK: - Goals

    func listGoals(status: GoalStatus? = nil, includeArchived: Bool = false) async throws -> GoalListResponse {
        var queryItems: [URLQueryItem] = []
        if let status, status != .unknown {
            queryItems.append(URLQueryItem(name: "status", value: status.rawValue))
        }
        if includeArchived {
            queryItems.append(URLQueryItem(name: "include_archived", value: "true"))
        }
        return try await self.fetch("/goals", queryItems: queryItems)
    }

    func createGoal(_ request: GoalCreateRequest) async throws -> Goal {
        try await self.fetch("/goals", method: "POST", body: request)
    }

    func updateGoal(_ id: String, _ request: GoalUpdateRequest) async throws -> Goal {
        try await self.fetch("/goals/\(id)", method: "PATCH", body: request)
    }

    func deleteGoal(_ id: String) async throws {
        try await self.fetchVoid("/goals/\(id)", method: "DELETE")
    }

    func completeGoal(_ id: String) async throws -> GoalActionResponse {
        try await self.fetch("/goals/\(id)/complete", method: "POST")
    }

    func archiveGoal(_ id: String) async throws -> GoalActionResponse {
        try await self.fetch("/goals/\(id)/archive", method: "POST")
    }

    // MARK: - Souls

    func listSouls() async throws -> SoulListResponse {
        try await self.fetch("/souls")
    }

    func createSoul(_ request: SoulCreateRequest) async throws -> Soul {
        try await self.fetch("/souls", method: "POST", body: request)
    }

    func updateSoul(_ id: String, _ request: SoulUpdateRequest) async throws -> Soul {
        try await self.fetch("/souls/\(id)", method: "PATCH", body: request)
    }

    func deleteSoul(_ id: String) async throws {
        try await self.fetchVoid("/souls/\(id)", method: "DELETE")
    }

    func enableSoul(_ id: String) async throws -> SoulActionResponse {
        try await self.fetch("/souls/\(id)/enable", method: "POST")
    }

    func disableSoul(_ id: String) async throws -> SoulActionResponse {
        try await self.fetch("/souls/\(id)/disable", method: "POST")
    }

    // MARK: - User Profiles

    func listUserProfiles() async throws -> UserProfileListResponse {
        try await self.fetch("/user-profiles")
    }

    func createUserProfile(_ request: UserProfileCreateRequest) async throws -> UserProfile {
        try await self.fetch("/user-profiles", method: "POST", body: request)
    }

    func updateUserProfile(_ id: String, _ request: UserProfileUpdateRequest) async throws -> UserProfile {
        try await self.fetch("/user-profiles/\(id)", method: "PATCH", body: request)
    }

    func deleteUserProfile(_ id: String) async throws {
        try await self.fetchVoid("/user-profiles/\(id)", method: "DELETE")
    }

    func enableUserProfile(_ id: String) async throws -> UserProfileActionResponse {
        try await self.fetch("/user-profiles/\(id)/enable", method: "POST")
    }

    func disableUserProfile(_ id: String) async throws -> UserProfileActionResponse {
        try await self.fetch("/user-profiles/\(id)/disable", method: "POST")
    }
}

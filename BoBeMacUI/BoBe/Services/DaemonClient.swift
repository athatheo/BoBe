import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "DaemonClient")

private struct DaemonErrorEnvelope: Decodable {
    let error: DaemonErrorBody
}

private struct DaemonErrorBody: Decodable {
    let message: String
}

actor DaemonClient {
    static let shared = DaemonClient()

    let baseURL: URL = {
        if let url = URL(string: DaemonConfig.baseURL) {
            return url
        }
        // Fallback only fires if `DaemonConfig.baseURL` ever stops parsing.
        // Both URL strings come from the same `DaemonConfig.{host,port}` so
        // this is unreachable today; left as defense in depth.
        logger.error("Invalid daemon base URL, falling back to localhost")
        return URL(string: DaemonConfig.baseURL) ?? URL(fileURLWithPath: "/")
    }()

    let session: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    let fetchTimeout: TimeInterval = 10

    private var sseTask: Task<Void, Never>?
    private var eventHandler: ((StreamBundle) -> Void)?
    private var connectionHandler: ((Bool) -> Void)?
    private var reconnectAttempts = 0
    private let maxReconnectAttempts = 10
    private var isReconnecting = false

    func endpointURL(_ path: String) -> URL {
        // appendingPathComponent percent-encodes `?` and `&`, which would corrupt query strings.
        // Split on `?` so the path is appended cleanly and the query is preserved.
        let trimmed = path.hasPrefix("/") ? String(path.dropFirst()) : path
        let (pathPart, queryPart): (String, String?)
        if let qIdx = trimmed.firstIndex(of: "?") {
            pathPart = String(trimmed[..<qIdx])
            queryPart = String(trimmed[trimmed.index(after: qIdx)...])
        } else {
            pathPart = trimmed
            queryPart = nil
        }
        var url = self.baseURL.appendingPathComponent(pathPart)
        if let queryPart, !queryPart.isEmpty {
            url = URL(string: "\(url.absoluteString)?\(queryPart)") ?? url
        }
        return url
    }

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
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 0

        do {
            let (bytes, response) = try await session.bytes(for: request)
            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200
            else {
                logger.warning("SSE connection failed with non-200 status")
                await self.handleSSEDisconnect()
                return
            }

            logger.info("SSE connected")
            self.reconnectAttempts = 0
            self.connectionHandler?(true)

            for try await line in bytes.lines {
                if Task.isCancelled { break }
                guard line.hasPrefix("data: ") else { continue }
                let jsonStr = String(line.dropFirst(6))
                guard let data = jsonStr.data(using: .utf8) else { continue }
                do {
                    let bundle = try decoder.decode(StreamBundle.self, from: data)
                    self.eventHandler?(bundle)
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
            await self.handleSSEDisconnect()
        }
    }

    private func handleSSEDisconnect() async {
        guard !self.isReconnecting else { return }
        self.isReconnecting = true
        defer { isReconnecting = false }

        self.connectionHandler?(false)
        self.reconnectAttempts += 1
        guard self.reconnectAttempts <= self.maxReconnectAttempts else {
            logger.error("Max SSE reconnect attempts reached")
            return
        }
        let delay = min(pow(2.0, Double(reconnectAttempts - 1)), 30.0)
        logger.info("SSE reconnecting in \(delay)s (attempt \(self.reconnectAttempts))")
        try? await Task.sleep(for: .seconds(delay))
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
        body: (any Encodable)?
    ) async throws -> Data {
        let url = self.endpointURL(path)
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = self.fetchTimeout

        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try self.encoder.encode(AnyEncodable(body))
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await self.session.data(for: request)
        } catch {
            logger.error("\(method) \(path): network error — \(error.localizedDescription)")
            throw error
        }
        guard let httpResponse = response as? HTTPURLResponse else {
            logger.error("\(method) \(path): invalid response (not HTTP)")
            throw DaemonError.invalidResponse
        }
        guard (200 ... 299).contains(httpResponse.statusCode) else {
            let message = self.errorMessage(from: data)
            logger.error("\(method) \(path) failed: HTTP \(httpResponse.statusCode) — \(message)")
            throw DaemonError.httpError(statusCode: httpResponse.statusCode, message: message)
        }
        return data
    }

    func fetch<T: Decodable>(
        _ path: String,
        method: String = "GET",
        body: (any Encodable)? = nil
    ) async throws -> T {
        let data = try await self.send(path, method: method, body: body)
        do {
            return try self.decoder.decode(T.self, from: data)
        } catch {
            logger.error("\(method) \(path): decode error — \(error.localizedDescription)")
            throw error
        }
    }

    func fetchVoid(
        _ path: String,
        method: String = "POST",
        body: (any Encodable)? = nil
    ) async throws {
        _ = try await self.send(path, method: method, body: body)
    }

    private func errorMessage(from data: Data) -> String {
        if let envelope = try? self.decoder.decode(DaemonErrorEnvelope.self, from: data) {
            return envelope.error.message
        }
        return String(data: data, encoding: .utf8) ?? "Unknown error"
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

    /// `/message` returns `{message_id}`; the message itself streams via SSE.
    @discardableResult
    func sendMessage(_ content: String) async throws -> SendMessageResponse {
        try await self.fetch("/message", method: "POST", body: SendMessageRequest(content: content))
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
        var query: [String] = []
        if let status, status != .unknown {
            query.append("status=\(status.rawValue)")
        }
        if includeArchived {
            query.append("include_archived=true")
        }
        let suffix = query.isEmpty ? "" : "?" + query.joined(separator: "&")
        return try await self.fetch("/goals\(suffix)")
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

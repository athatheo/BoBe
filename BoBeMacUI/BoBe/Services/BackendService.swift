import Darwin
import Foundation
import os
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BackendService")

enum ServiceState: Sendable {
    case stopped, starting, ready, crashed, fatal
}

private enum ManagedProcessIdentity {
    case verifiedDaemon(path: String)
    case otherProcess(path: String)
    case notRunning
    case unverifiable
}

/// Seconds we wait between SIGTERM and SIGKILL during graceful daemon
/// stop. Past this the daemon is considered wedged.
private let terminateGraceSeconds: TimeInterval = 12
/// Initial poll cadence after spawn; ramps geometrically toward the cap.
private let healthCheckInitialDelay: TimeInterval = 0.2
/// Multiplier applied to the poll cadence after each failure.
private let healthCheckBackoffMultiplier: Double = 1.5
/// Maximum delay between consecutive health checks.
private let healthCheckMaxDelay: TimeInterval = 5.0
/// Total number of health-check attempts before giving up.
private let healthCheckMaxAttempts = 30
/// In-memory stderr ring capacity for the daemon child; older lines are
/// discarded so a stuck process doesn't bloat the host.
private let stderrBufferMaxLines = 50

actor BackendService {
    static let shared = BackendService()

    private var process: Process?
    private var state: ServiceState = .stopped
    private var stopping = false
    private var restartCount = 0
    private var lifecycleGeneration: UInt64 = 0
    private var restartTask: Task<Void, Never>?
    private let maxRestartAttempts = 3
    private let dataDir: URL
    private let pidFilePath: URL
    private(set) var lastError: String?
    /// Non-fatal — daemon /health OK but a subsystem reported degraded.
    private(set) var startupWarning: String?
    /// Multi-cast continuations — every `stateStream` subscriber gets its
    /// own continuation appended here, so two consumers (`BobeStore` and
    /// `SettingsStore`) both receive every transition. Prior implementation
    /// was a single `AsyncStream` with one buffered continuation, which is
    /// single-subscriber — whichever consumer hit `next()` first won every
    /// value and the other starved. `SettingsStore`'s "refetch on daemon
    /// recovery" silently never fired under that model.
    private var stateContinuations: [UUID: AsyncStream<ServiceState>.Continuation] = [:]

    private init() {
        self.dataDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(BobePaths.dataDirName)
        self.pidFilePath = self.dataDir.appendingPathComponent("bobe-service.pid")
    }

    /// Returns a fresh `AsyncStream<ServiceState>` for each subscriber. The
    /// continuation is registered in `stateContinuations` and removed on
    /// stream termination so subscribers that go away don't leak.
    nonisolated var stateStream: AsyncStream<ServiceState> {
        AsyncStream { continuation in
            let id = UUID()
            Task { await self.registerContinuation(id: id, continuation: continuation) }
            continuation.onTermination = { @Sendable _ in
                Task { await self.unregisterContinuation(id: id) }
            }
        }
    }

    private func registerContinuation(
        id: UUID,
        continuation: AsyncStream<ServiceState>.Continuation
    ) {
        self.stateContinuations[id] = continuation
        // Emit the current state on subscribe so late subscribers see the
        // present state, not just future transitions.
        continuation.yield(self.state)
    }

    private func unregisterContinuation(id: UUID) {
        self.stateContinuations.removeValue(forKey: id)
    }

    private func transition(to newState: ServiceState) {
        self.state = newState
        for cont in self.stateContinuations.values {
            cont.yield(newState)
        }
    }

    func start() async throws {
        guard DaemonConfig.endpoint.managesLocalProcess else {
            self.transition(to: .starting)
            _ = try await DaemonClient.shared.health()
            self.transition(to: .ready)
            return
        }
        guard self.state != .starting, self.state != .ready else { return }
        self.lifecycleGeneration &+= 1
        self.restartTask?.cancel()
        self.restartTask = nil
        self.stopping = false

        await self.cleanStalePID()
        try self.createDataDirIfNeeded()

        self.transition(to: .starting)
        do {
            try await self.spawnAndWaitHealthy()
        } catch {
            await self.stop()
            throw error
        }
    }

    func stop() async {
        guard DaemonConfig.endpoint.managesLocalProcess else {
            self.transition(to: .stopped)
            return
        }
        self.lifecycleGeneration &+= 1
        self.restartTask?.cancel()
        self.restartTask = nil
        self.stopping = true
        if self.process == nil {
            self.cleanup()
            return
        }
        guard let proc = process, proc.isRunning else {
            self.cleanup()
            return
        }

        logger.info("Stopping bobe backend (PID: \(proc.processIdentifier))")
        proc.terminate()

        let deadline = Date.now.addingTimeInterval(terminateGraceSeconds)
        while proc.isRunning, Date.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
        }

        if proc.isRunning {
            logger.warning("bobe backend didn't exit gracefully, sending SIGKILL")
            kill(proc.processIdentifier, SIGKILL)
        }

        self.cleanup()
        logger.info("bobe backend stopped")
    }

    /// User-initiated restart from the overlay's "Daemon down" CTA. Stops
    /// the current process (if any) then starts fresh. Resets the auto-
    /// restart counter so a manual retry isn't blocked by the backoff.
    func userRestart() async throws {
        self.restartCount = 0
        await self.stop()
        try await self.start()
    }

    // MARK: - Spawn & Health

    private func spawnAndWaitHealthy() async throws {
        let binaryPath = self.findBinaryPath()
        guard let binaryPath else {
            if ProcessInfo.processInfo.environment["BOBE_DEV"] != nil {
                logger.info("bobe binary not found — dev mode, expecting manual backend")
                self.transition(to: .ready)
                return
            }
            self.transition(to: .fatal)
            self.lastError = "Backend binary not found in app bundle. Try reinstalling BoBe."
            throw BackendServiceError.spawnFailed("bobe-daemon binary not found")
        }

        logger.info("Starting bobe backend: \(binaryPath, privacy: .public)")
        self.lastError = nil
        self.startupWarning = nil

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binaryPath)
        proc.arguments = [
            "serve",
            "--host", DaemonConfig.defaultHost,
            "--port", String(DaemonConfig.defaultPort),
        ]
        proc.currentDirectoryURL = self.dataDir

        var env = ProcessInfo.processInfo.environment
        env["HOME"] = FileManager.default.homeDirectoryForCurrentUser.path
        env["BOBE_DATA_DIR"] = self.dataDir.path
        guard let copilotCLIPath = Self.resolveCopilotCLIPath(
            environment: env,
            bundleURL: Bundle.main.bundleURL,
            isExecutableFile: { FileManager.default.isExecutableFile(atPath: $0) }
        ) else {
            self.transition(to: .fatal)
            self.lastError = "Copilot CLI helper is missing. Try reinstalling BoBe."
            throw BackendServiceError.spawnFailed("Copilot CLI helper not found")
        }
        env["COPILOT_CLI_PATH"] = copilotCLIPath
        proc.environment = env

        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe

        let stderrBuf = StderrBuffer()
        outPipe.fileHandleForReading.readabilityHandler = { handle in
            if let line = String(data: handle.availableData, encoding: .utf8), !line.isEmpty {
                logger.info("[bobe-service] \(line.trimmingCharacters(in: .newlines), privacy: .public)")
            }
        }
        errPipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if let line = String(data: data, encoding: .utf8), !line.isEmpty {
                let trimmed = line.trimmingCharacters(in: .newlines)
                logger.error("[bobe-service] \(trimmed, privacy: .public)")
                stderrBuf.append(trimmed)
            }
        }

        if self.isPortInUse(DaemonConfig.defaultPort) {
            await self.cleanStalePID()

            if self.isPortInUse(DaemonConfig.defaultPort) {
                self.lastError =
                    "Port \(DaemonConfig.defaultPort) is already in use by another application. "
                        + "Close the conflicting application, then relaunch BoBe."
                throw BackendServiceError.healthCheckFailed
            }
        }

        do {
            try proc.run()
        } catch {
            self.transition(to: .fatal)
            throw BackendServiceError.spawnFailed(error.localizedDescription)
        }

        self.process = proc
        self.writePID(proc.processIdentifier)

        proc.terminationHandler = { [weak self] terminatedProc in
            Task { [weak self] in
                await self?.handleExit(
                    pid: terminatedProc.processIdentifier,
                    exitCode: Int(terminatedProc.terminationStatus)
                )
            }
        }

        do {
            try await self.waitForHealth()
        } catch {
            let captured = stderrBuf.text
            if !captured.isEmpty {
                self.lastError = captured
                logger.error("Backend stderr on failure: \(captured, privacy: .public)")
            }
            throw error
        }
        self.transition(to: .ready)
        self.restartCount = 0
        logger.info("bobe backend healthy (PID: \(proc.processIdentifier))")
    }

    /// Daemon returns 200 even on degraded DB — inspect body, not status.
    private func waitForHealth() async throws {
        var delay = healthCheckInitialDelay

        for attempt in 1 ... healthCheckMaxAttempts {
            if self.stopping { throw BackendServiceError.stoppedDuringHealthCheck }
            if self.process?.isRunning != true { throw BackendServiceError.processExitedDuringHealthCheck }

            do {
                let response = try await DaemonClient.shared.health()
                if let services = response.services, services.database != "ok" {
                    logger.warning("Backend database degraded: \(services.database, privacy: .public)")
                    self.startupWarning = L10n.tr("app.service_warning.database_degraded")
                }
                return
            } catch {
                logger.debug("Health check attempt \(attempt)/\(healthCheckMaxAttempts) failed, retrying in \(delay)s")
                try await Task.sleep(for: .seconds(delay))
                delay = min(delay * healthCheckBackoffMultiplier, healthCheckMaxDelay)
            }
        }
        throw BackendServiceError.healthCheckFailed
    }

    // MARK: - Crash Recovery

    private func handleExit(pid: Int32, exitCode: Int) {
        guard self.process?.processIdentifier == pid else {
            logger.debug("Ignoring stale termination callback for PID \(pid)")
            return
        }
        self.process = nil
        try? FileManager.default.removeItem(at: self.pidFilePath)

        guard !self.stopping else { return }

        logger.warning("bobe backend exited unexpectedly (code: \(exitCode))")
        if self.state == .starting {
            self.transition(to: .stopped)
            return
        }
        self.transition(to: .crashed)

        self.scheduleAutomaticRestart()
    }

    private func scheduleAutomaticRestart() {
        self.restartCount += 1
        if self.restartCount > self.maxRestartAttempts {
            logger.error("bobe backend failed \(self.maxRestartAttempts) times, giving up")
            self.transition(to: .fatal)
            return
        }

        let backoffSeconds = self.restartCount
        logger.info("Restarting bobe backend in \(backoffSeconds)s (attempt \(self.restartCount)/\(self.maxRestartAttempts))")

        let generation = self.lifecycleGeneration
        self.restartTask?.cancel()
        self.restartTask = Task { [weak self] in
            do {
                try await Task.sleep(for: .seconds(backoffSeconds))
            } catch {
                return
            }
            guard let self else { return }
            await self.runAutomaticRestart(generation: generation)
        }
    }

    private func runAutomaticRestart(generation: UInt64) async {
        guard generation == self.lifecycleGeneration, !self.stopping else { return }
        do {
            self.transition(to: .starting)
            try await self.spawnAndWaitHealthy()
            self.restartTask = nil
        } catch {
            logger.error("Restart failed: \(error.localizedDescription, privacy: .public)")
            guard generation == self.lifecycleGeneration else { return }
            if let proc = self.process, proc.isRunning {
                proc.terminate()
            }
            self.process = nil
            self.transition(to: .crashed)
            self.scheduleAutomaticRestart()
        }
    }

    // MARK: - PID File Management

    private func writePID(_ pid: Int32) {
        try? "\(pid)".write(to: self.pidFilePath, atomically: true, encoding: .utf8)
    }

    private func cleanStalePID() async {
        guard let pidStr = try? String(contentsOf: pidFilePath, encoding: .utf8),
              let pid = Int32(pidStr.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            if self.isPortInUse(DaemonConfig.defaultPort) {
                logger.warning("Port \(DaemonConfig.defaultPort) is in use but no PID file exists — another process may be bound")
            }
            return
        }

        switch self.inspectManagedProcess(pid: pid) {
        case let .verifiedDaemon(path):
            logger.info("Cleaning stale bobe daemon (PID: \(pid), path: \(path, privacy: .public))")
            kill(pid, SIGTERM)
            try? await Task.sleep(for: .seconds(2))
            if kill(pid, 0) == 0 {
                kill(pid, SIGKILL)
            }
        case let .otherProcess(path):
            logger.warning(
                "PID file points to a different live process (PID: \(pid), path: \(path, privacy: .public)); refusing to terminate it"
            )
            if self.isPortInUse(DaemonConfig.defaultPort) {
                logger.warning("Port \(DaemonConfig.defaultPort) remains in use by another process")
            }
        case .notRunning:
            if self.isPortInUse(DaemonConfig.defaultPort) {
                logger.warning("Port \(DaemonConfig.defaultPort) in use but PID \(pid) from PID file is not running — stale PID file")
            }
        case .unverifiable:
            logger.warning("Could not verify PID \(pid) from PID file as bobe-daemon; refusing to terminate it")
            if self.isPortInUse(DaemonConfig.defaultPort) {
                logger.warning("Port \(DaemonConfig.defaultPort) remains in use by another process")
            }
        }
        try? FileManager.default.removeItem(at: self.pidFilePath)
    }

    private func inspectManagedProcess(pid: Int32) -> ManagedProcessIdentity {
        guard kill(pid, 0) == 0 else { return .notRunning }
        guard let path = self.processPath(for: pid) else { return .unverifiable }
        return self.isManagedDaemonPath(path) ? .verifiedDaemon(path: path) : .otherProcess(path: path)
    }

    private func processPath(for pid: Int32) -> String? {
        var buffer = [CChar](repeating: 0, count: Int(MAXPATHLEN))
        let written = buffer.withUnsafeMutableBytes { rawBuffer in
            proc_pidpath(pid, rawBuffer.baseAddress, UInt32(rawBuffer.count))
        }
        guard written > 0 else { return nil }
        let bytes = buffer.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }
        return String(bytes: bytes, encoding: .utf8)
    }

    private func isManagedDaemonPath(_ path: String) -> Bool {
        guard let standardized = self.canonicalizedPath(path),
              let binaryPath = self.findBinaryPath(),
              let managedPath = self.canonicalizedPath(binaryPath)
        else {
            return false
        }

        return standardized == managedPath
    }

    private func canonicalizedPath(_ path: String) -> String? {
        URL(fileURLWithPath: path)
            .standardizedFileURL
            .resolvingSymlinksInPath()
            .path
    }

    private func isPortInUse(_ port: Int) -> Bool {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { close(fd) }

        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = UInt16(port).bigEndian
        addr.sin_addr.s_addr = inet_addr("127.0.0.1")

        let result = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockPtr in
                connect(fd, sockPtr, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        return result == 0
    }

    private func cleanup() {
        self.process = nil
        self.transition(to: .stopped)
        try? FileManager.default.removeItem(at: self.pidFilePath)
    }

    private func createDataDirIfNeeded() throws {
        try FileManager.default.createDirectory(at: self.dataDir, withIntermediateDirectories: true)
    }

    // MARK: - Binary Discovery

    nonisolated static func resolveCopilotCLIPath(
        environment: [String: String],
        bundleURL: URL,
        isExecutableFile: (String) -> Bool
    ) -> String? {
        let helperPath = bundleURL
            .appendingPathComponent("Contents/Helpers/copilot", isDirectory: false)
            .path
        if isExecutableFile(helperPath) {
            return helperPath
        }

        if let configuredPath = environment["COPILOT_CLI_PATH"],
           !configuredPath.isEmpty,
           isExecutableFile(configuredPath) {
            return configuredPath
        }
        return nil
    }

    private func findBinaryPath() -> String? {
        if let execURL = Bundle.main.executableURL {
            let siblingPath = execURL.deletingLastPathComponent()
                .appendingPathComponent("bobe-daemon").path
            if FileManager.default.isExecutableFile(atPath: siblingPath) {
                return siblingPath
            }
        }

        if let bundlePath = Bundle.main.path(forResource: "bobe-daemon", ofType: nil) {
            return bundlePath
        }

        let devPaths = [
            FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".cargo/bin/bobe").path,
            "/usr/local/bin/bobe",
        ]
        for path in devPaths where FileManager.default.isExecutableFile(atPath: path) {
            return path
        }
        return nil
    }
}

enum BackendServiceError: Error, LocalizedError {
    case healthCheckFailed
    case spawnFailed(String)
    case stoppedDuringHealthCheck
    case processExitedDuringHealthCheck

    var errorDescription: String? {
        switch self {
        case .healthCheckFailed: "Backend health check failed after maximum attempts"
        case let .spawnFailed(msg): "Failed to start backend: \(msg)"
        case .stoppedDuringHealthCheck: "Service was stopped during health check"
        case .processExitedDuringHealthCheck: "Backend process exited during health check"
        }
    }
}

private final class StderrBuffer: Sendable {
    private let lines = OSAllocatedUnfairLock(initialState: [String]())

    func append(_ line: String) {
        self.lines.withLock { lines in
            lines.append(line)
            if lines.count > stderrBufferMaxLines {
                lines.removeFirst(lines.count - stderrBufferMaxLines)
            }
        }
    }

    var text: String {
        self.lines.withLock { $0.joined(separator: "\n") }
    }
}

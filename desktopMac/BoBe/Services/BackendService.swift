import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BackendService")

enum ServiceState: Sendable {
    case stopped, starting, ready, crashed, fatal
}

actor BackendService {
    static let shared = BackendService()

    private var process: Process?
    private var state: ServiceState = .stopped
    private var stopping = false
    private var restartCount = 0
    private let maxRestartAttempts = 3
    private let dataDir: URL
    private let pidFilePath: URL
    private(set) var lastError: String?
    private var stateContinuation: AsyncStream<ServiceState>.Continuation?
    nonisolated let stateStream: AsyncStream<ServiceState>

    private init() {
        var cont: AsyncStream<ServiceState>.Continuation?
        self.stateStream = AsyncStream { cont = $0 }
        self.stateContinuation = cont
        self.dataDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".bobe")
        self.pidFilePath = self.dataDir.appendingPathComponent("bobe-service.pid")
    }

    private func transition(to newState: ServiceState) {
        self.state = newState
        self.stateContinuation?.yield(newState)
    }

    func start() async throws {
        guard self.state != .starting, self.state != .ready else { return }
        self.stopping = false

        await self.cleanStalePID()
        try self.createDataDirIfNeeded()

        self.transition(to: .starting)
        try await self.spawnAndWaitHealthy()
    }

    func stop() async {
        self.stopping = true
        if self.process == nil {
            await self.cleanStalePID()
            self.cleanup()
            return
        }
        guard let proc = process, proc.isRunning else {
            self.cleanup()
            return
        }

        logger.info("Stopping bobe backend (PID: \(proc.processIdentifier))")
        proc.terminate()

        let deadline = Date().addingTimeInterval(12)
        while proc.isRunning, Date() < deadline {
            try? await Task.sleep(for: .milliseconds(100))
        }

        if proc.isRunning {
            logger.warning("bobe backend didn't exit gracefully, sending SIGKILL")
            kill(proc.processIdentifier, SIGKILL)
        }

        self.cleanup()
        logger.info("bobe backend stopped")
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

        logger.info("Starting bobe backend: \(binaryPath)")
        self.lastError = nil

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binaryPath)
        proc.arguments = ["serve"]
        proc.currentDirectoryURL = self.dataDir

        var env = ProcessInfo.processInfo.environment
        env["HOME"] = FileManager.default.homeDirectoryForCurrentUser.path
        env["BOBE_DATA_DIR"] = self.dataDir.path
        proc.environment = env

        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe

        let stderrBuf = StderrBuffer()
        outPipe.fileHandleForReading.readabilityHandler = { handle in
            if let line = String(data: handle.availableData, encoding: .utf8), !line.isEmpty {
                logger.info("[bobe-service] \(line.trimmingCharacters(in: .newlines))")
            }
        }
        errPipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if let line = String(data: data, encoding: .utf8), !line.isEmpty {
                let trimmed = line.trimmingCharacters(in: .newlines)
                logger.error("[bobe-service] \(trimmed)")
                stderrBuf.append(trimmed)
            }
        }

        if self.isPortInUse(DaemonConfig.port) {
            await self.cleanStalePID()

            if self.isPortInUse(DaemonConfig.port) {
                self.lastError =
                    "Port \(DaemonConfig.port) is already in use by another application. "
                        + "Close the conflicting app or set BOBE_PORT to a different port."
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
                await self?.handleExit(exitCode: Int(terminatedProc.terminationStatus))
            }
        }

        do {
            try await self.waitForHealth()
        } catch {
            let captured = stderrBuf.text
            if !captured.isEmpty {
                self.lastError = captured
                logger.error("Backend stderr on failure: \(captured)")
            }
            throw error
        }
        self.transition(to: .ready)
        self.restartCount = 0
        logger.info("bobe backend healthy (PID: \(proc.processIdentifier))")
    }

    /// Exponential backoff health polling.
    private func waitForHealth() async throws {
        var delay: TimeInterval = 0.2
        let maxAttempts = 30

        for attempt in 1 ... maxAttempts {
            if self.stopping { throw BackendServiceError.stoppedDuringHealthCheck }
            if self.process?.isRunning != true { throw BackendServiceError.processExitedDuringHealthCheck }

            do {
                _ = try await DaemonClient.shared.health()
                return
            } catch {
                logger.debug("Health check attempt \(attempt)/\(maxAttempts) failed, retrying in \(delay)s")
                try await Task.sleep(for: .seconds(delay))
                delay = min(delay * 1.5, 5.0)
            }
        }
        throw BackendServiceError.healthCheckFailed
    }

    // MARK: - Crash Recovery

    private func handleExit(exitCode: Int) {
        guard !self.stopping else { return }

        logger.warning("bobe backend exited unexpectedly (code: \(exitCode))")
        self.transition(to: .crashed)

        self.restartCount += 1
        if self.restartCount > self.maxRestartAttempts {
            logger.error("bobe backend failed \(self.maxRestartAttempts) times, giving up")
            self.transition(to: .fatal)
            return
        }

        let backoffSeconds = self.restartCount
        logger.info("Restarting bobe backend in \(backoffSeconds)s (attempt \(self.restartCount)/\(self.maxRestartAttempts))")

        Task {
            try? await Task.sleep(for: .seconds(backoffSeconds))
            guard !self.stopping else { return }
            do {
                try await self.spawnAndWaitHealthy()
            } catch {
                logger.error("Restart failed: \(error.localizedDescription)")
                self.transition(to: .fatal)
            }
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
            if self.isPortInUse(DaemonConfig.port) {
                logger.warning("Port \(DaemonConfig.port) is in use but no PID file exists — another process may be bound")
            }
            return
        }

        if kill(pid, 0) == 0 {
            logger.info("Cleaning stale process (PID: \(pid))")
            kill(pid, SIGTERM)
            try? await Task.sleep(for: .seconds(2))
            if kill(pid, 0) == 0 {
                kill(pid, SIGKILL)
            }
        } else if self.isPortInUse(DaemonConfig.port) {
            logger.warning("Port \(DaemonConfig.port) in use but PID \(pid) from PID file is not running — stale PID file")
        }
        try? FileManager.default.removeItem(at: self.pidFilePath)
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

private final class StderrBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var lines: [String] = []

    func append(_ line: String) {
        self.lock.lock()
        defer { lock.unlock() }
        self.lines.append(line)
        if self.lines.count > 50 { self.lines.removeFirst(self.lines.count - 50) }
    }

    var text: String {
        self.lock.lock()
        defer { lock.unlock() }
        return self.lines.joined(separator: "\n")
    }
}

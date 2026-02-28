import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BackendService")

/// Service states matching original python-service.ts
enum ServiceState: Sendable {
    case stopped, starting, ready, crashed, fatal
}

/// Manages the bobe backend binary lifecycle with crash recovery.
/// Based on original python-service.ts — 3-attempt restart, PID file, health checks.
actor BackendService {
    static let shared = BackendService()

    private var process: Process?
    private var state: ServiceState = .stopped
    private var stopping = false
    private var restartCount = 0
    private let maxRestartAttempts = 3
    private let dataDir: URL
    private let pidFilePath: URL
    /// Last stderr output captured from a failed backend launch.
    private(set) var lastError: String?

    var isReady: Bool { state == .ready }
    var isFatal: Bool { state == .fatal }
    var currentState: ServiceState { state }

    private init() {
        dataDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".bobe")
        pidFilePath = dataDir.appendingPathComponent("bobe-service.pid")
    }

    /// Start the backend with up to 3 retry attempts on failure.
    func start() async throws {
        guard state != .starting && state != .ready else { return }
        stopping = false

        await cleanStalePID()
        try createDataDirIfNeeded()

        state = .starting
        try await spawnAndWaitHealthy()
    }

    /// Gracefully stop the backend
    func stop() async {
        stopping = true
        if process == nil {
            // Try PID file as fallback
            await cleanStalePID()
            cleanup()
            return
        }
        guard let proc = process, proc.isRunning else {
            cleanup()
            return
        }

        logger.info("Stopping bobe backend (PID: \(proc.processIdentifier))")
        proc.terminate()

        // Backend needs up to ~12s for graceful shutdown (MCP + Ollama unload + DB close)
        let deadline = Date().addingTimeInterval(12)
        while proc.isRunning && Date() < deadline {
            try? await Task.sleep(for: .milliseconds(100))
        }

        if proc.isRunning {
            logger.warning("bobe backend didn't exit gracefully, sending SIGKILL")
            kill(proc.processIdentifier, SIGKILL)
        }

        cleanup()
        logger.info("bobe backend stopped")
    }

    // MARK: - Spawn & Health

    private func spawnAndWaitHealthy() async throws {
        let binaryPath = findBinaryPath()
        guard let binaryPath else {
            logger.warning("bobe binary not found — running in dev mode")
            state = .ready
            return
        }

        logger.info("Starting bobe backend: \(binaryPath)")
        lastError = nil

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binaryPath)
        proc.arguments = ["serve"]
        proc.currentDirectoryURL = dataDir

        // Only pass BOBE_DATA_DIR — backend derives all paths internally
        var env = ProcessInfo.processInfo.environment
        env["HOME"] = FileManager.default.homeDirectoryForCurrentUser.path
        env["BOBE_DATA_DIR"] = dataDir.path
        proc.environment = env

        // Capture stdout/stderr for logging
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe

        // Accumulate stderr so we can surface it on failure
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

        // Check for port conflict before launching
        if isPortInUse(DaemonConfig.port) {
            await cleanStalePID()

            if isPortInUse(DaemonConfig.port) {
                lastError = "Port \(DaemonConfig.port) is already in use by another application. "
                    + "Close the conflicting app or set BOBE_PORT to a different port."
                throw BackendServiceError.healthCheckFailed
            }
        }

        do {
            try proc.run()
        } catch {
            state = .fatal
            throw BackendServiceError.spawnFailed(error.localizedDescription)
        }

        self.process = proc
        writePID(proc.processIdentifier)

        // Monitor for unexpected exit
        proc.terminationHandler = { [weak self] terminatedProc in
            Task { [weak self] in
                await self?.handleExit(exitCode: Int(terminatedProc.terminationStatus))
            }
        }

        // Wait for health
        do {
            try await waitForHealth()
        } catch {
            let captured = stderrBuf.text
            if !captured.isEmpty {
                lastError = captured
                logger.error("Backend stderr on failure: \(captured)")
            }
            throw error
        }
        state = .ready
        restartCount = 0
        logger.info("bobe backend healthy (PID: \(proc.processIdentifier))")
    }

    /// Exponential backoff health check — production pattern
    private func waitForHealth() async throws {
        var delay: TimeInterval = 0.2
        let maxAttempts = 30

        for attempt in 1...maxAttempts {
            if stopping { throw BackendServiceError.stoppedDuringHealthCheck }
            if process?.isRunning != true { throw BackendServiceError.processExitedDuringHealthCheck }

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
        guard !stopping else { return }

        logger.warning("bobe backend exited unexpectedly (code: \(exitCode))")
        state = .crashed

        restartCount += 1
        if restartCount > maxRestartAttempts {
            logger.error("bobe backend failed \(self.maxRestartAttempts) times, giving up")
            state = .fatal
            return
        }

        let backoffSeconds = restartCount
        logger.info("Restarting bobe backend in \(backoffSeconds)s (attempt \(self.restartCount)/\(self.maxRestartAttempts))")

        Task {
            try? await Task.sleep(for: .seconds(backoffSeconds))
            guard !stopping else { return }
            do {
                try await spawnAndWaitHealthy()
            } catch {
                logger.error("Restart failed: \(error.localizedDescription)")
                state = .fatal
            }
        }
    }

    // MARK: - PID File Management

    private func writePID(_ pid: Int32) {
        try? "\(pid)".write(to: pidFilePath, atomically: true, encoding: .utf8)
    }

    private func cleanStalePID() async {
        guard let pidStr = try? String(contentsOf: pidFilePath, encoding: .utf8),
              let pid = Int32(pidStr.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            // No PID file — check if something else holds the port
            if isPortInUse(DaemonConfig.port) {
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
        } else if isPortInUse(DaemonConfig.port) {
            logger.warning("Port \(DaemonConfig.port) in use but PID \(pid) from PID file is not running — stale PID file")
        }
        try? FileManager.default.removeItem(at: pidFilePath)
    }

    /// Check whether a TCP port is already bound on localhost.
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
        process = nil
        state = .stopped
        try? FileManager.default.removeItem(at: pidFilePath)
    }

    private func createDataDirIfNeeded() throws {
        try FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
    }

    // MARK: - Binary Discovery

    private func findBinaryPath() -> String? {
        // 1. Side-by-side in Contents/MacOS/ (production bundled location)
        //    Named "bobe-daemon" to avoid case-insensitive collision with "BoBe" on APFS.
        if let execURL = Bundle.main.executableURL {
            let siblingPath = execURL.deletingLastPathComponent()
                .appendingPathComponent("bobe-daemon").path
            if FileManager.default.isExecutableFile(atPath: siblingPath) {
                return siblingPath
            }
        }

        // 2. Bundle resource fallback
        if let bundlePath = Bundle.main.path(forResource: "bobe-daemon", ofType: nil) {
            return bundlePath
        }

        // 3. Dev fallbacks (original binary name)
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
    case binaryNotFound
    case spawnFailed(String)
    case stoppedDuringHealthCheck
    case processExitedDuringHealthCheck

    var errorDescription: String? {
        switch self {
        case .healthCheckFailed: "Backend health check failed after maximum attempts"
        case .binaryNotFound: "Could not find bobe binary"
        case .spawnFailed(let msg): "Failed to start backend: \(msg)"
        case .stoppedDuringHealthCheck: "Service was stopped during health check"
        case .processExitedDuringHealthCheck: "Backend process exited during health check"
        }
    }
}

/// Thread-safe buffer for accumulating stderr output from the backend process.
private final class StderrBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var lines: [String] = []

    func append(_ line: String) {
        lock.lock()
        defer { lock.unlock() }
        lines.append(line)
        // Keep only last 50 lines to avoid unbounded growth
        if lines.count > 50 { lines.removeFirst(lines.count - 50) }
    }

    var text: String {
        lock.lock()
        defer { lock.unlock() }
        return lines.joined(separator: "\n")
    }
}

import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "BackendService")

/// Service states matching Electron python-service.ts
enum ServiceState: Sendable {
    case stopped, starting, ready, crashed, fatal
}

/// Manages the bobe backend binary lifecycle with crash recovery.
/// Equivalent to Electron's python-service.ts — 3-attempt restart, PID file, health checks.
actor BackendService {
    static let shared = BackendService()

    private var process: Process?
    private var state: ServiceState = .stopped
    private var stopping = false
    private var restartCount = 0
    private let maxRestartAttempts = 3
    private let dataDir: URL
    private let pidFilePath: URL

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

        cleanStalePID()
        try createDataDirIfNeeded()

        state = .starting
        try await spawnAndWaitHealthy()
    }

    /// Gracefully stop the backend
    func stop() async {
        stopping = true
        guard let proc = process, proc.isRunning else {
            cleanup()
            return
        }

        logger.info("Stopping bobe backend (PID: \(proc.processIdentifier))")
        proc.terminate()

        let deadline = Date().addingTimeInterval(5)
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

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binaryPath)
        proc.arguments = ["serve"]

        var env = ProcessInfo.processInfo.environment
        env["BOBE_HOST"] = DaemonConfig.host
        env["BOBE_PORT"] = "\(DaemonConfig.port)"
        let dbPath = dataDir.appendingPathComponent("bobe.db").path
        env["BOBE_DATABASE_URL"] = "sqlite:\(dbPath)"
        let ollamaDir = dataDir.appendingPathComponent("ollama/bin").path
        env["PATH"] = [ollamaDir, env["PATH"] ?? ""].joined(separator: ":")
        env["OLLAMA_HOST"] = "127.0.0.1:11434"
        env["OLLAMA_ORIGINS"] = "http://127.0.0.1:*"
        env["OLLAMA_MODELS"] = dataDir.appendingPathComponent("models").path
        proc.environment = env

        // Capture stdout/stderr for logging
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        outPipe.fileHandleForReading.readabilityHandler = { handle in
            if let line = String(data: handle.availableData, encoding: .utf8), !line.isEmpty {
                logger.info("[bobe-service] \(line.trimmingCharacters(in: .newlines))")
            }
        }
        errPipe.fileHandleForReading.readabilityHandler = { handle in
            if let line = String(data: handle.availableData, encoding: .utf8), !line.isEmpty {
                logger.error("[bobe-service] \(line.trimmingCharacters(in: .newlines))")
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
        try await waitForHealth()
        state = .ready
        restartCount = 0
        logger.info("bobe backend healthy (PID: \(proc.processIdentifier))")
    }

    /// Exponential backoff health check — matches Electron pattern
    private func waitForHealth() async throws {
        var delay: TimeInterval = 0.2
        let maxAttempts = 30

        for attempt in 1...maxAttempts {
            if stopping { throw BackendServiceError.stoppedDuringHealthCheck }
            if process?.isRunning != true { throw BackendServiceError.processExitedDuringHealthCheck }

            do {
                let _ = try await DaemonClient.shared.health()
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

    private func cleanStalePID() {
        guard let pidStr = try? String(contentsOf: pidFilePath, encoding: .utf8),
              let pid = Int32(pidStr.trimmingCharacters(in: .whitespacesAndNewlines)) else { return }

        if kill(pid, 0) == 0 {
            logger.info("Cleaning stale process (PID: \(pid))")
            kill(pid, SIGTERM)
            usleep(2_000_000) // 2s
            if kill(pid, 0) == 0 {
                kill(pid, SIGKILL)
            }
        }
        try? FileManager.default.removeItem(at: pidFilePath)
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
        if let bundlePath = Bundle.main.path(forResource: "bobe", ofType: nil) {
            return bundlePath
        }
        let devPaths = [
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".cargo/bin/bobe").path,
            "/usr/local/bin/bobe"
        ]
        for path in devPaths {
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
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

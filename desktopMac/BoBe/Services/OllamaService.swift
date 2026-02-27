import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "OllamaService")

/// Manages Ollama binary download, verification, and lifecycle.
/// Mirrors Python OllamaManager: health check → auto-start → auto-pull.
/// Everything lives under ~/.bobe/ollama/ (bin, models, logs).
actor OllamaService {
    static let shared = OllamaService()

    private let ollamaVersion = "v0.17.0"
    private let bobeDir: URL
    private let ollamaDir: URL
    private let binDir: URL
    private let modelsDir: URL

    private init() {
        bobeDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".bobe")
        ollamaDir = bobeDir.appendingPathComponent("ollama")
        binDir = ollamaDir.appendingPathComponent("bin")
        modelsDir = bobeDir.appendingPathComponent("models")
    }

    /// The binary path inside ~/.bobe/ollama/bin/ollama
    var binaryPath: String { binDir.appendingPathComponent("ollama").path }

    /// Download Ollama if not already present, or find system Ollama
    func ensureInstalled(onProgress: @Sendable @escaping (Double, String) -> Void) async throws -> String {
        // 1. Check our managed binary
        if FileManager.default.isExecutableFile(atPath: binaryPath) {
            logger.info("Ollama already installed at \(self.binaryPath)")
            return binaryPath
        }

        // 2. Check for system-installed Ollama (Homebrew, manual)
        for systemPath in ["/opt/homebrew/bin/ollama", "/usr/local/bin/ollama"] {
            if FileManager.default.isExecutableFile(atPath: systemPath) {
                logger.info("System Ollama found at \(systemPath)")
                return systemPath
            }
        }

        // 3. Download and extract
        try FileManager.default.createDirectory(at: binDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: modelsDir, withIntermediateDirectories: true)

        guard let downloadURL = URL(string: "https://github.com/ollama/ollama/releases/download/\(ollamaVersion)/ollama-darwin.tgz") else {
            throw OllamaError.invalidURL("release_download")
        }
        logger.info("Downloading Ollama \(self.ollamaVersion) from \(downloadURL)")
        onProgress(0, "Downloading Ollama \(ollamaVersion)...")

        let (tempURL, response) = try await URLSession.shared.download(from: downloadURL)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            throw OllamaError.downloadFailed
        }

        onProgress(50, "Extracting Ollama...")

        // Extract — tarball contains ollama binary + dylibs at root level
        let extract = Process()
        extract.executableURL = URL(fileURLWithPath: "/usr/bin/tar")
        extract.arguments = ["-xzf", tempURL.path, "-C", binDir.path]
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            extract.terminationHandler = { proc in
                if proc.terminationStatus == 0 {
                    cont.resume()
                } else {
                    cont.resume(throwing: OllamaError.extractionFailed)
                }
            }
            do { try extract.run() }
            catch { cont.resume(throwing: error) }
        }

        // Verify the binary exists after extraction
        guard FileManager.default.isExecutableFile(atPath: binaryPath) else {
            logger.error("Ollama binary not found at \(self.binaryPath) after extraction")
            throw OllamaError.extractionFailed
        }

        onProgress(100, "Ollama ready")
        logger.info("Ollama \(self.ollamaVersion) installed at \(self.binaryPath)")
        return binaryPath
    }

    /// Start Ollama server if not already running.
    /// Configures OLLAMA_MODELS to ~/.bobe/models so everything stays under .bobe.
    func start(binaryPath: String) async throws -> Process? {
        if await isOllamaResponding() {
            logger.info("Ollama already running — skipping start")
            return nil
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: binaryPath)
        process.arguments = ["serve"]

        // Keep all Ollama data under ~/.bobe/
        var env = ProcessInfo.processInfo.environment
        env["OLLAMA_HOST"] = "127.0.0.1:11434"
        env["OLLAMA_ORIGINS"] = "http://127.0.0.1:*"
        env["OLLAMA_MODELS"] = modelsDir.path
        process.environment = env

        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        logger.info("Ollama server started (PID: \(process.processIdentifier))")
        return process
    }

    /// Poll Ollama /api/tags until it responds (up to 30 seconds)
    func waitUntilReady() async -> Bool {
        for _ in 0..<60 {
            if await isOllamaResponding() { return true }
            try? await Task.sleep(for: .milliseconds(500))
        }
        return false
    }

    private func isOllamaResponding() async -> Bool {
        guard let url = URL(string: "http://127.0.0.1:11434/api/tags") else {
            return false
        }
        var request = URLRequest(url: url)
        request.timeoutInterval = 2
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            return (response as? HTTPURLResponse)?.statusCode == 200
        } catch {
            return false
        }
    }
}

enum OllamaError: Error, LocalizedError {
    case downloadFailed
    case extractionFailed
    case invalidURL(String)

    var errorDescription: String? {
        switch self {
        case .downloadFailed: "Failed to download Ollama"
        case .extractionFailed: "Failed to extract Ollama — binary not found after download"
        case .invalidURL(let context): "Invalid URL for \(context)"
        }
    }
}

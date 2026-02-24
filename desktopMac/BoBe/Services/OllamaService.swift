import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "OllamaService")

/// Manages Ollama binary download, verification, and lifecycle.
/// Based on original ollama-service.ts.
actor OllamaService {
    static let shared = OllamaService()

    private let ollamaVersion = "v0.6.2"
    private let downloadDir: URL

    private init() {
        let appSupport = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".bobe")
        self.downloadDir = appSupport.appendingPathComponent("ollama")
    }

    /// Download Ollama if not already present, or check for system Ollama
    func ensureInstalled(onProgress: @Sendable @escaping (Double, String) -> Void) async throws -> String {
        let binaryPath = downloadDir.appendingPathComponent("ollama").path

        if FileManager.default.isExecutableFile(atPath: binaryPath) {
            logger.info("Ollama already installed at \(binaryPath)")
            return binaryPath
        }

        // Check for system-installed Ollama (e.g., via Homebrew)
        for systemPath in ["/usr/local/bin/ollama", "/opt/homebrew/bin/ollama"] {
            if FileManager.default.isExecutableFile(atPath: systemPath) {
                logger.info("System Ollama found at \(systemPath)")
                return systemPath
            }
        }

        try FileManager.default.createDirectory(at: downloadDir, withIntermediateDirectories: true)

        let downloadURL = ollamaDownloadURL()
        logger.info("Downloading Ollama from \(downloadURL)")

        onProgress(0, "Downloading Ollama...")

        let (tempURL, response) = try await URLSession.shared.download(from: downloadURL)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            throw OllamaError.downloadFailed
        }

        onProgress(50, "Extracting...")

        // Extract tarball
        let extractProcess = Process()
        extractProcess.executableURL = URL(fileURLWithPath: "/usr/bin/tar")
        extractProcess.arguments = ["-xzf", tempURL.path, "-C", downloadDir.path]
        try extractProcess.run()
        extractProcess.waitUntilExit()

        guard extractProcess.terminationStatus == 0 else {
            throw OllamaError.extractionFailed
        }

        // Make executable
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o755],
            ofItemAtPath: binaryPath
        )

        onProgress(100, "Ollama ready")
        logger.info("Ollama installed at \(binaryPath)")
        return binaryPath
    }

    /// Start Ollama server if not already running
    func start(binaryPath: String) async throws -> Process? {
        // Check if Ollama is already serving (e.g., system install or previous run)
        if await isOllamaResponding() {
            logger.info("Ollama already running — skipping start")
            return nil
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: binaryPath)
        process.arguments = ["serve"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        logger.info("Ollama server started (PID: \(process.processIdentifier))")
        return process
    }

    /// Poll Ollama until it responds (up to 15 seconds)
    func waitUntilReady() async -> Bool {
        for _ in 0..<30 {
            if await isOllamaResponding() { return true }
            try? await Task.sleep(for: .milliseconds(500))
        }
        return false
    }

    private func isOllamaResponding() async -> Bool {
        let url = URL(string: "http://127.0.0.1:11434/api/tags")!
        var request = URLRequest(url: url)
        request.timeoutInterval = 2
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            return (response as? HTTPURLResponse)?.statusCode == 200
        } catch {
            return false
        }
    }

    private func ollamaDownloadURL() -> URL {
        #if arch(arm64)
        let arch = "arm64"
        #else
        let arch = "amd64"
        #endif
        return URL(string: "https://github.com/ollama/ollama/releases/download/\(ollamaVersion)/ollama-darwin-\(arch).tgz")!
    }
}

enum OllamaError: Error, LocalizedError {
    case downloadFailed
    case extractionFailed
    case verificationFailed

    var errorDescription: String? {
        switch self {
        case .downloadFailed: "Failed to download Ollama"
        case .extractionFailed: "Failed to extract Ollama archive"
        case .verificationFailed: "Ollama checksum verification failed"
        }
    }
}

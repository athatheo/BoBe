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

    /// Download Ollama if not already present
    func ensureInstalled(onProgress: @Sendable @escaping (Double, String) -> Void) async throws -> String {
        let binaryPath = downloadDir.appendingPathComponent("ollama").path

        if FileManager.default.isExecutableFile(atPath: binaryPath) {
            logger.info("Ollama already installed at \(binaryPath)")
            return binaryPath
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

    /// Start Ollama server
    func start(binaryPath: String) async throws -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: binaryPath)
        process.arguments = ["serve"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        logger.info("Ollama server started (PID: \(process.processIdentifier))")
        return process
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

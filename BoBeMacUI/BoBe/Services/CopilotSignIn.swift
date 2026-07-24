import Foundation

/// Expert-mode Terminal fallback for the installed CLI's device flow.
/// Normal sign-in stays inside BoBe through the daemon-owned PTY flow.
enum CopilotSignIn {
    static func openLogin(cliPath: String?) {
        let resolvedPath = cliPath ?? BackendService.resolveCopilotCLIPath(
            environment: ProcessInfo.processInfo.environment,
            bundleURL: Bundle.main.bundleURL,
            isExecutableFile: { FileManager.default.isExecutableFile(atPath: $0) }
        )
        let command = self.loginCommand(cliPath: resolvedPath)
        let script = """
        tell application "Terminal"
            activate
            do script "\(command)"
        end tell
        """
        let process = Process()
        process.launchPath = "/usr/bin/osascript"
        process.arguments = ["-e", script]
        try? process.run()
    }

    static func loginCommand(cliPath: String?) -> String {
        let executable: String
        if let cliPath {
            let escaped = cliPath.replacingOccurrences(of: "'", with: "'\\''")
            executable = "'\(escaped)'"
        } else {
            executable = "copilot"
        }
        return "\(executable) --no-auto-update login"
    }
}

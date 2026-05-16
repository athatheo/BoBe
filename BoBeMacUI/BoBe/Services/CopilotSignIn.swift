import Foundation

/// Terminal.app handoff for the bundled `copilot` CLI's device-flow prompt
/// (interactive flow can't be replicated inside the app process). Living
/// in Services/ — orchestration, not view code.
enum CopilotSignIn {
    static func openLogin(cliPath: String?) {
        let command: String
        if let cliPath {
            let escaped = cliPath.replacingOccurrences(of: "'", with: "'\\''")
            command = "'\(escaped)'"
        } else {
            command = "copilot"
        }
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
}

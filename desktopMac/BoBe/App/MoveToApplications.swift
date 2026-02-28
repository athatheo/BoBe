import AppKit
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "MoveToApplications")

/// Prompts the user to move the app to /Applications if it's running from
/// a read-only volume (DMG), ~/Downloads, or /tmp.
@MainActor
func moveToApplicationsIfNeeded() {
    let bundlePath = Bundle.main.bundlePath

    // Already in /Applications or ~/Applications — nothing to do
    let homeApps = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Applications").path
    if bundlePath.hasPrefix("/Applications/") || bundlePath.hasPrefix(homeApps + "/") {
        return
    }

    // Only prompt when running from locations that indicate a non-installed state
    let shouldPrompt = isReadOnlyVolume(bundlePath)
        || bundlePath.contains("/Downloads/")
        || bundlePath.hasPrefix("/tmp/")
        || bundlePath.hasPrefix("/private/tmp/")
    guard shouldPrompt else { return }

    logger.info("App running from \(bundlePath) — prompting to move to Applications")

    let alert = NSAlert()
    alert.messageText = "BoBe works best from the Applications folder."
    alert.informativeText = "Would you like to move BoBe there now?"
    alert.alertStyle = .informational
    alert.addButton(withTitle: "Move to Applications")
    alert.addButton(withTitle: "Not Now")

    let response = alert.runModal()
    guard response == .alertFirstButtonReturn else { return }

    let destPath = "/Applications/BoBe.app"
    let fm = FileManager.default

    do {
        // Remove existing copy if present
        if fm.fileExists(atPath: destPath) {
            try fm.removeItem(atPath: destPath)
        }
        try fm.copyItem(atPath: bundlePath, toPath: destPath)
        logger.info("Copied app bundle to \(destPath)")
    } catch {
        logger.error("Failed to copy app to Applications: \(error.localizedDescription)")
        let errAlert = NSAlert()
        errAlert.messageText = "Couldn't move BoBe"
        errAlert.informativeText = error.localizedDescription
        errAlert.alertStyle = .warning
        errAlert.runModal()
        NSApp.terminate(nil)
        return
    }

    // Relaunch from new location
    let destURL = URL(fileURLWithPath: destPath)
    let config = NSWorkspace.OpenConfiguration()
    config.createsNewApplicationInstance = true
    NSWorkspace.shared.openApplication(at: destURL, configuration: config) { _, error in
        if let error {
            logger.error("Failed to relaunch from Applications: \(error.localizedDescription)")
        }
    }
    NSApp.terminate(nil)
}

/// Returns true if the path resides on a read-only filesystem (e.g. a mounted DMG).
private func isReadOnlyVolume(_ path: String) -> Bool {
    var stat = statfs()
    guard statfs(path, &stat) == 0 else { return false }
    return (Int32(stat.f_flags) & MNT_RDONLY) != 0
}

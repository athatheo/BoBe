import AppKit
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "MoveToApplications")

@MainActor
func moveToApplicationsIfNeeded() {
    let bundlePath = Bundle.main.bundlePath

    let homeApps = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Applications").path
    if bundlePath.hasPrefix("/Applications/") || bundlePath.hasPrefix(homeApps + "/") {
        return
    }

    let shouldPrompt =
        isReadOnlyVolume(bundlePath)
            || bundlePath.contains("/Downloads/")
            || bundlePath.hasPrefix("/tmp/")
            || bundlePath.hasPrefix("/private/tmp/")
    guard shouldPrompt else { return }

    logger.info("App running from \(bundlePath) — prompting to move to Applications")

    let alert = NSAlert()
    alert.messageText = L10n.tr("app.move_to_applications.title")
    alert.informativeText = L10n.tr("app.move_to_applications.message")
    alert.alertStyle = .informational
    alert.addButton(withTitle: L10n.tr("app.move_to_applications.confirm"))
    alert.addButton(withTitle: L10n.tr("app.move_to_applications.cancel"))

    let response = alert.runModal()
    guard response == .alertFirstButtonReturn else { return }

    let destPath = "/Applications/BoBe.app"
    let fm = FileManager.default

    do {
        if fm.fileExists(atPath: destPath) {
            try fm.removeItem(atPath: destPath)
        }
        try fm.copyItem(atPath: bundlePath, toPath: destPath)
        logger.info("Copied app bundle to \(destPath)")
    } catch {
        logger.error("Failed to copy app to Applications: \(error.localizedDescription)")
        let errAlert = NSAlert()
        errAlert.messageText = L10n.tr("app.move_to_applications.error_title")
        errAlert.informativeText = error.localizedDescription
        errAlert.alertStyle = .warning
        errAlert.runModal()
        NSApp.terminate(nil)
        return
    }

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

private func isReadOnlyVolume(_ path: String) -> Bool {
    var stat = statfs()
    guard statfs(path, &stat) == 0 else { return false }
    return (Int32(stat.f_flags) & MNT_RDONLY) != 0
}

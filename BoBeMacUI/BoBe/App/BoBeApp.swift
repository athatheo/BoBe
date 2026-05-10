import AppKit
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "App")

extension Notification.Name {
    static let bobeCaptureStateChanged = Notification.Name("bobe.captureStateChanged")
}

@main
struct BoBeApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        Settings {
            EmptyView()
        }
        .commands {
            CommandGroup(replacing: .appSettings) {
                Button(L10n.tr("tray.settings")) {
                    SettingsWindowManager.shared.show()
                }
                .keyboardShortcut(",", modifiers: .command)
            }
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let store: BobeStore
    private let trayManager: TrayManager
    private var isQuitting = false
    private var isStartingUp = true

    override init() {
        let store = BobeStore.shared
        self.store = store
        self.trayManager = TrayManager(store: store)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        logger.info("BoBe starting up")

        self.store.applyPersistedLocale()

        NSApp.setActivationPolicy(.regular)

        let currentPID = ProcessInfo.processInfo.processIdentifier
        let bundleMatches = NSRunningApplication.runningApplications(
            withBundleIdentifier: "com.bobe.app"
        )
        .filter { $0.processIdentifier != currentPID }

        var candidates = bundleMatches
        for app in NSWorkspace.shared.runningApplications
            where app.processIdentifier != currentPID
            && (app.executableURL?.lastPathComponent == "BoBe" || app.localizedName == "BoBe")
            && !candidates.contains(where: { $0.processIdentifier == app.processIdentifier }) {
            candidates.append(app)
        }

        if let existing = candidates.first {
            logger.warning("Another BoBe instance detected (pid: \(existing.processIdentifier)) — activating it and exiting")
            existing.activate()
            NSApp.terminate(nil)
            return
        }

        moveToApplicationsIfNeeded()

        Task { @MainActor in
            self.setDockIcon()
        }

        self.trayManager.setup()
        UpdaterManager.shared.setup()

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleWelcomeCompleted),
            name: .bobeWelcomeCompleted,
            object: nil
        )

        Task { @MainActor in
            await self.startApp()
        }
    }

    @objc
    private func handleWelcomeCompleted() {
        Task { @MainActor in
            SetupWindowManager.shared.close()
            self.showOverlay()
            self.store.connect()
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !self.isQuitting else { return .terminateNow }
        self.isQuitting = true

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(20))
            logger.warning("Hard shutdown timeout — forcing exit")
            exit(0)
        }

        Task {
            self.store.beginShutdown()

            try? await Task.sleep(for: .milliseconds(600))

            self.store.disconnect()
            await BackendService.shared.stop()

            await MainActor.run {
                OverlayWindowManager.shared.close()
                SettingsWindowManager.shared.close()
                NSApp.reply(toApplicationShouldTerminate: true)
            }
        }
        return .terminateLater
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            OverlayWindowManager.shared.show()
        }
        return true
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        if OverlayWindowManager.shared.panel == nil, !self.isQuitting, !self.isStartingUp {
            Task { @MainActor in
                self.showOverlay()
            }
        }
    }

    @MainActor
    private func startApp() async {
        defer { self.isStartingUp = false }
        let isDev = ProcessInfo.processInfo.environment["BOBE_DEV"] != nil

        if !isDev {
            var serviceStarted = false
            var attempt = 0
            while !serviceStarted {
                attempt += 1
                do {
                    try await BackendService.shared.start()
                    serviceStarted = true
                } catch {
                    logger.error("Backend start attempt \(attempt) failed: \(error.localizedDescription)")
                    if attempt >= 3 {
                        let shouldRetry = await showServiceErrorDialog(error: error)
                        guard shouldRetry else {
                            NSApp.terminate(nil)
                            return
                        }
                        attempt = 0
                        continue
                    }
                }
            }
        } else {
            logger.info("Dev mode: skipping service management (run `bobe serve` manually)")
        }

        // After backend is healthy, surface any non-fatal startup warning
        // (e.g. degraded database) into the store before connecting SSE.
        if let warning = await BackendService.shared.startupWarning {
            self.store.surfaceWarning(warning)
        }

        if SetupWindowManager.shared.isOnboardingCompleted {
            self.showOverlay()
            self.store.connect()
        } else {
            SetupWindowManager.shared.show()
            // store.connect() defers until handleWelcomeCompleted observer fires
        }
    }

    @MainActor
    private func showOverlay() {
        let theme = ThemeStore.shared.currentTheme
        let overlayView = OverlayView(store: self.store)
            .environment(\.theme, theme)

        OverlayWindowManager.shared.createPanel(with: overlayView)
    }

    @MainActor
    private func showServiceErrorDialog(error: Error) async -> Bool {
        var detail =
            L10n.tr("app.service_error.detail_prefix")
                + "\(error.localizedDescription)\n\n"
        if let stderr = await BackendService.shared.lastError {
            detail += L10n.tr("app.service_error.backend_output_prefix") + "\(stderr)\n\n"
        }
        detail += L10n.tr("app.service_error.logs_hint")

        let alert = NSAlert()
        alert.messageText = L10n.tr("app.service_error.title")
        alert.informativeText = detail
        alert.alertStyle = .critical
        alert.addButton(withTitle: L10n.tr("app.common.retry"))
        alert.addButton(withTitle: L10n.tr("app.common.quit"))
        return alert.runModal() == .alertFirstButtonReturn
    }

    @MainActor
    private func setDockIcon() {
        let isDark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let iconName = isDark ? "bobe_app_dock_dark" : "bobe_app_dock_light"
        if let iconURL = Bundle.appResources.url(forResource: iconName, withExtension: "png"),
           let icon = NSImage(contentsOf: iconURL) {
            NSApp.applicationIconImage = icon
        }
    }
}

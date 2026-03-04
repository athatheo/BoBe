import AppKit
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "App")

extension Notification.Name {
    static let bobeSetupCompleted = Notification.Name("bobe.setupCompleted")
}

@main
struct BoBeApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        Settings {
            EmptyView()
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var isQuitting = false
    private var isStartingUp = true
    private var setupWindow: NSWindow?
    private var setupCompletedObserver: Any?
    private var isShowingSetupAlert = false
    private var isTransitioningFromSetup = false
    private var setupCloseCount = 0
    var setupCompletedSuccessfully = false

    @MainActor
    func completeSetupAndCloseWizard() {
        logger.info("setup.complete_handoff.begin")
        self.setupCompletedSuccessfully = true
        self.unregisterSetupCloseObserver(for: self.setupWindow)

        let windowToHide = self.setupWindow ?? NSApplication.shared.keyWindow
        windowToHide?.animationBehavior = .none
        windowToHide?.orderOut(nil)
        self.setupWindow = nil
        self.isTransitioningFromSetup = true

        Task { @MainActor [weak self] in
            guard let self else { return }
            try? await Task.sleep(for: .milliseconds(150))
            self.showOverlay()
            BobeStore.shared.connect()
            self.setupCompletedSuccessfully = false
            self.isTransitioningFromSetup = false
            logger.info("setup.complete_handoff.end")
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        logger.info("BoBe starting up")

        if self.setupCompletedObserver == nil {
            self.setupCompletedObserver = NotificationCenter.default.addObserver(
                forName: .bobeSetupCompleted,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.completeSetupAndCloseWizard()
                }
            }
        }

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

        TrayManager.shared.setup()
        UpdaterManager.shared.setup()

        Task { @MainActor in
            await self.startApp()
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !self.isQuitting else { return .terminateNow }
        self.isQuitting = true

        DispatchQueue.main.asyncAfter(deadline: .now() + 20) {
            logger.warning("Hard shutdown timeout — forcing exit")
            exit(0)
        }

        Task {
            BobeStore.shared.beginShutdown()

            try? await Task.sleep(for: .milliseconds(600))

            BobeStore.shared.disconnect()
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
        if OverlayWindowManager.shared.panel == nil, self.setupWindow == nil, !self.isQuitting, !self.isShowingSetupAlert,
           !self.isTransitioningFromSetup, !self.isStartingUp {
            Task { @MainActor in
                self.showOverlay()
            }
        }
    }

    @MainActor
    private func startApp() async {
        defer { self.isStartingUp = false }
        let isDev = ProcessInfo.processInfo.environment["BOBE_DEV"] != nil
        let forceOnboarding = ProcessInfo.processInfo.environment["BOBE_FORCE_ONBOARDING"] == "1"

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

        if forceOnboarding {
            logger.info("BOBE_FORCE_ONBOARDING=1 — forcing wizard")
            self.showSetupWizard()
            return
        }

        do {
            let status = try await DaemonClient.shared.getOnboardingStatus()
            if status.needsOnboarding {
                self.showSetupWizard()
                return
            }
        } catch {
            logger.error("Could not check onboarding status: \(error.localizedDescription)")
            let shouldRetry = await showBackendErrorDialog(message: error.localizedDescription)
            if !shouldRetry {
                NSApp.terminate(nil)
                return
            }
        }

        self.showOverlay()
        BobeStore.shared.connect()
    }

    @MainActor
    private func showOverlay() {
        let theme = ThemeStore.shared.currentTheme
        let overlayView = OverlayView()
            .environment(\.theme, theme)

        OverlayWindowManager.shared.createPanel(with: overlayView)
    }

    @MainActor
    private func showSetupWizard() {
        // Close any existing overlay — only one UI path should be active
        OverlayWindowManager.shared.close()

        self.unregisterSetupCloseObserver(for: self.setupWindow)
        self.setupCompletedSuccessfully = false

        let theme = ThemeStore.shared.currentTheme
        let setupView = SetupWizard()
            .environment(\.theme, theme)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 540, height: 700),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "BoBe Setup"
        window.titlebarAppearsTransparent = true
        window.isMovableByWindowBackground = true
        window.animationBehavior = .none
        window.center()
        window.setContentSize(NSSize(width: 540, height: 700))
        window.contentViewController = NSHostingController(rootView: setupView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()

        self.setupWindow = window
        self.registerSetupCloseObserver(for: window)
    }

    @objc
    private func handleSetupWindowWillClose(_ notification: Notification) {
        guard let closedWindow = notification.object as? NSWindow,
              closedWindow == self.setupWindow,
              !self.isQuitting else { return }

        self.unregisterSetupCloseObserver(for: closedWindow)
        self.setupWindow = nil

        if self.setupCompletedSuccessfully {
            logger.info("setup.wizard_closed_after_success")
            self.setupCompletedSuccessfully = false
            self.setupCloseCount = 0
            return
        }

        self.setupCloseCount += 1
        if self.setupCloseCount >= 3 {
            logger.info("setup.wizard_closed_3x_showing_escape")
            self.isShowingSetupAlert = true
            let alert = NSAlert()
            alert.messageText = "Setup incomplete"
            alert.informativeText = "BoBe needs to be configured before it can work."
            alert.alertStyle = .warning
            alert.addButton(withTitle: "Retry Setup")
            alert.addButton(withTitle: "Open Settings")
            alert.addButton(withTitle: "Quit")
            let response = alert.runModal()
            self.isShowingSetupAlert = false
            switch response {
            case .alertFirstButtonReturn:
                self.setupCloseCount = 0
                self.showSetupWizard()
            case .alertSecondButtonReturn:
                self.showOverlay()
                BobeStore.shared.connect()
                SettingsWindowManager.shared.show()
            default:
                NSApp.terminate(nil)
            }
            return
        }

        logger.info("setup.wizard_closed_incomplete_reopening (\(self.setupCloseCount)/3)")
        self.showSetupWizard()
    }

    private func registerSetupCloseObserver(for window: NSWindow) {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleSetupWindowWillClose(_:)),
            name: NSWindow.willCloseNotification,
            object: window
        )
    }

    private func unregisterSetupCloseObserver(for window: NSWindow?) {
        guard let window else { return }
        NotificationCenter.default.removeObserver(
            self,
            name: NSWindow.willCloseNotification,
            object: window
        )
    }

    @MainActor
    private func showServiceErrorDialog(error: Error) async -> Bool {
        var detail =
            "The backend service failed to start after multiple attempts.\n\n"
                + "\(error.localizedDescription)\n\n"
        if let stderr = await BackendService.shared.lastError {
            detail += "Backend output:\n\(stderr)\n\n"
        }
        detail += "Logs are in ~/.bobe/logs/"

        let alert = NSAlert()
        alert.messageText = "BoBe couldn't start"
        alert.informativeText = detail
        alert.alertStyle = .critical
        alert.addButton(withTitle: "Retry")
        alert.addButton(withTitle: "Quit")
        return alert.runModal() == .alertFirstButtonReturn
    }

    @MainActor
    private func showBackendErrorDialog(message: String) async -> Bool {
        let alert = NSAlert()
        alert.messageText = "BoBe backend error"
        alert.informativeText =
            "BoBe could not verify backend readiness.\n\n\(message)\n\nRetry or quit."
        alert.alertStyle = .critical
        alert.addButton(withTitle: "Retry")
        alert.addButton(withTitle: "Quit")
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

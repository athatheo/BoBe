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
    private var setupWindow: NSWindow?
    private var setupCloseObserver: Any?
    private var setupCompletedObserver: Any?
    private var isShowingSetupAlert = false
    /// Set by SetupWizard when onboarding completes successfully — prevents
    /// the willCloseNotification handler from showing "Setup isn't complete".
    var setupCompletedSuccessfully = false

    @MainActor
    func completeSetupAndCloseWizard() {
        logger.info("setup.complete_handoff.begin")
        setupCompletedSuccessfully = true
        if let observer = setupCloseObserver {
            NotificationCenter.default.removeObserver(observer)
            setupCloseObserver = nil
        }

        let windowToHide = setupWindow ?? NSApplication.shared.keyWindow
        windowToHide?.animationBehavior = .none
        windowToHide?.orderOut(nil)
        setupWindow = nil

        Task { @MainActor [weak self] in
            guard let self else { return }
            try? await Task.sleep(for: .milliseconds(150))
            NSApp.setActivationPolicy(.accessory)
            self.showOverlay()
            BobeStore.shared.connect()
            self.setupCompletedSuccessfully = false
            logger.info("setup.complete_handoff.end")
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        logger.info("BoBe starting up")

        if setupCompletedObserver == nil {
            setupCompletedObserver = NotificationCenter.default.addObserver(
                forName: .bobeSetupCompleted,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.completeSetupAndCloseWizard()
                }
            }
        }

        NSApp.setActivationPolicy(.accessory)

        // Single instance guard — activate existing instance and bail.
        // In dev, bundle ID can be missing; add executable/name fallback.
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

        // Prompt to move to /Applications if launched from DMG / Downloads / tmp
        moveToApplicationsIfNeeded()

        // Set dock icon based on theme
        Task { @MainActor in
            self.setDockIcon()
        }

        TrayManager.shared.setup()

        Task { @MainActor in
            await startApp()
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !isQuitting else { return .terminateNow }
        isQuitting = true

        // 20-second hard timeout guarantees exit (12s grace + 600ms UI + margin)
        DispatchQueue.main.asyncAfter(deadline: .now() + 20) {
            logger.warning("Hard shutdown timeout — forcing exit")
            exit(0)
        }

        Task {
            // Show shutting-down state in overlay before teardown
            BobeStore.shared.beginShutdown()

            // Brief pause so the user sees the goodbye state
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
        // Only recreate overlay if panel doesn't exist and we're not in setup or alert flow
        if OverlayWindowManager.shared.panel == nil && setupWindow == nil && !isQuitting && !isShowingSetupAlert {
            Task { @MainActor in
                showOverlay()
            }
        }
    }

    @MainActor
    private func startApp() async {
        let isDev = ProcessInfo.processInfo.environment["BOBE_DEV"] != nil
        let forceOnboarding = ProcessInfo.processInfo.environment["BOBE_FORCE_ONBOARDING"] == "1"

        // Production: start backend with retry dialog
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
                        if shouldRetry {
                            attempt = 0
                            continue
                        } else {
                            NSApp.terminate(nil)
                            return
                        }
                    }
                }
            }
        } else {
            logger.info("Dev mode: skipping service management (run `bobe serve` manually)")
        }

        // Check onboarding status — works in both dev and production mode
        if forceOnboarding {
            logger.info("BOBE_FORCE_ONBOARDING=1 — forcing wizard")
            showSetupWizard()
            return
        }

        do {
            let status = try await DaemonClient.shared.getOnboardingStatus()
            if status.needsOnboarding {
                showSetupWizard()
                return
            }
        } catch {
            logger.warning("Could not check onboarding status: \(error.localizedDescription)")
        }

        // Onboarding not needed — show overlay and connect SSE
        showOverlay()
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
        // Clean up any previous observer
        if let observer = setupCloseObserver {
            NotificationCenter.default.removeObserver(observer)
            setupCloseObserver = nil
        }
        setupCompletedSuccessfully = false

        // Temporarily show in dock so wizard can activate properly
        NSApp.setActivationPolicy(.regular)

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
        window.contentView = NSHostingView(rootView: setupView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        // Retain the window so ARC doesn't release it
        self.setupWindow = window

        setupCloseObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self, !self.isQuitting else { return }

                if let observer = self.setupCloseObserver {
                    NotificationCenter.default.removeObserver(observer)
                    self.setupCloseObserver = nil
                }

                self.setupWindow = nil
                NSApp.setActivationPolicy(.accessory)

                if self.setupCompletedSuccessfully {
                    logger.info("setup.wizard_closed_after_success")
                    self.setupCompletedSuccessfully = false
                    return
                }

                logger.info("setup.wizard_closed_incomplete_reopening")
                DispatchQueue.main.async { [weak self] in
                    self?.showSetupWizard()
                }
            }
        }
    }

    @MainActor
    private func showServiceErrorDialog(error: Error) async -> Bool {
        var detail = "The backend service failed to start after multiple attempts.\n\n"
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
    private func setDockIcon() {
        let isDark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let iconName = isDark ? "bobe_app_dock_dark" : "bobe_app_dock_light"
        if let iconURL = Bundle.appResources.url(forResource: iconName, withExtension: "png"),
           let icon = NSImage(contentsOf: iconURL) {
            NSApp.applicationIconImage = icon
        }
    }
}

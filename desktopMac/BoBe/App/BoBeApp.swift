import SwiftUI
import AppKit
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "App")

@main
struct BoBeApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        Settings {
            EmptyView()
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate, @unchecked Sendable {
    private var isQuitting = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        logger.info("BoBe starting up")

        NSApp.setActivationPolicy(.accessory)

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

        // 8-second hard timeout guarantees exit (matches Electron pattern)
        DispatchQueue.main.asyncAfter(deadline: .now() + 8) {
            logger.warning("Hard shutdown timeout — forcing exit")
            exit(0)
        }

        Task {
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

    func applicationDidBecomeActive(_ notification: Notification) {
        // Recreate overlay if no windows exist (macOS standard)
        if NSApp.windows.filter({ $0.isVisible }).isEmpty && !isQuitting {
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
        let theme = ThemeStore.shared.currentTheme
        let setupView = SetupWizard()
            .environment(\.theme, theme)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 540, height: 620),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "BoBe Setup"
        window.titlebarAppearsTransparent = true
        window.isMovableByWindowBackground = true
        window.center()
        window.setContentSize(NSSize(width: 540, height: 620))
        window.contentView = NSHostingView(rootView: setupView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard self?.isQuitting != true else { return }
                self?.showOverlay()
                BobeStore.shared.connect()
            }
        }
    }

    @MainActor
    private func showServiceErrorDialog(error: Error) async -> Bool {
        let alert = NSAlert()
        alert.messageText = "BoBe couldn't start"
        alert.informativeText = "The backend service failed to start after multiple attempts.\n\n\(error.localizedDescription)\n\nLogs are in ~/.bobe/logs/"
        alert.alertStyle = .critical
        alert.addButton(withTitle: "Retry")
        alert.addButton(withTitle: "Quit")
        return alert.runModal() == .alertFirstButtonReturn
    }

    @MainActor
    private func setDockIcon() {
        // Use theme-appropriate dock icon from resources
        let isDark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let iconName = isDark ? "bobe_app_dock_dark" : "bobe_app_dock_light"
        if let iconPath = Bundle.main.path(forResource: iconName, ofType: "png"),
           let icon = NSImage(contentsOfFile: iconPath) {
            NSApp.applicationIconImage = icon
        }
    }
}

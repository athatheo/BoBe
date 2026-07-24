import AppKit
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "App")

@main
struct BoBeApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var store = BobeStore.shared
    @State private var voicePipeline = VoicePipeline.shared

    var body: some Scene {
        // The persistent menu-bar item lives as a declarative SwiftUI scene
        // now (`BobeMenuBarScene`) — replaces the previous TrayManager +
        // NSStatusItem duo. Sees locale overrides, state changes, and the
        // overlay-visibility flag through @Observable singletons.
        BobeMenuBarScene(store: self.store)

        Settings {
            EmptyView()
        }
        .commands {
            CommandGroup(replacing: .appSettings) {
                Button(L10n.tr("tray.settings")) {
                    if SetupWindowManager.shared.isOnboardingCompleted {
                        SettingsWindowManager.shared.show()
                    } else {
                        SetupWindowManager.shared.show()
                    }
                }
                .keyboardShortcut(",", modifiers: .command)
            }
            // View menu — overlay visibility is the only window-level toggle
            // the app exposes (no document windows). Sits next to the system
            // View menu items rather than under our own custom CommandMenu so
            // it appears in the conventional location for window-visibility
            // commands.
            CommandGroup(after: .toolbar) {
                Button(
                    SetupWindowManager.shared.isOnboardingCompleted
                        ? L10n.tr("menu.view.toggle_overlay")
                        : L10n.tr("app.setup_incomplete.retry")
                ) {
                    if SetupWindowManager.shared.isOnboardingCompleted {
                        OverlayWindowManager.shared.toggle()
                    } else {
                        SetupWindowManager.shared.show()
                    }
                }
                .keyboardShortcut("b", modifiers: [.command, .shift])
            }
            // Voice menu — Toggle Mic and Stop Speaking. Stop Speaking uses
            // Cmd+. (the macOS-conventional cancel shortcut) so it works even
            // when the chat input has focus.
            CommandMenu(L10n.tr("menu.voice.title")) {
                Button(L10n.tr("menu.voice.toggle_mic")) {
                    Task { @MainActor in
                        guard let url = URL(string: DaemonConfig.baseURL) else { return }
                        await self.voicePipeline.toggle(daemonBaseURL: url)
                    }
                }
                .keyboardShortcut("m", modifiers: [.command, .shift])

                Button(L10n.tr("menu.voice.stop_speaking")) {
                    self.voicePipeline.interrupt()
                }
                .keyboardShortcut(".", modifiers: .command)
            }
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let store: BobeStore
    private var isQuitting = false
    private var isDuplicateInstance = false
    private var isStartingUp = true
    private let isBodyAdapterOnly =
        ProcessInfo.processInfo.environment["BOBE_BODY_ADAPTER_ONLY"] == "1"

    override init() {
        self.store = BobeStore.shared
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        logger.info("BoBe starting up")

        if self.isBodyAdapterOnly {
            NSApp.setActivationPolicy(.prohibited)
            self.isStartingUp = false
            guard let configuration = BodyAdapterConfig.load() else {
                logger.error("Body adapter-only mode has invalid configuration")
                NSApp.terminate(nil)
                return
            }
            Task {
                await BodySpeechAdapter.shared.start(configuration: configuration)
            }
            return
        }

        self.store.applyPersistedLocale()

        NSApp.setActivationPolicy(.accessory)

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
            self.isDuplicateInstance = true
            existing.activate()
            NSApp.terminate(nil)
            return
        }

        moveToApplicationsIfNeeded()

        // Tray is now a declarative `MenuBarExtra` scene
        // (`BobeMenuBarScene`) registered in the App body — no AppDelegate
        // setup call required. The scene's lifecycle is automatic.
        UpdaterManager.shared.setup()
        SystemPowerObserver.shared.start()

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(self.handleWelcomeCompleted),
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
        if self.isDuplicateInstance { return .terminateNow }
        guard !self.isQuitting else { return .terminateNow }
        self.isQuitting = true
        if self.isBodyAdapterOnly {
            Task {
                await BodySpeechAdapter.shared.stop()
                await MainActor.run {
                    NSApp.reply(toApplicationShouldTerminate: true)
                }
            }
            return .terminateLater
        }

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(20))
            logger.warning("Hard shutdown timeout — forcing exit")
            exit(0)
        }

        Task {
            self.store.beginShutdown()
            await SettingsStore.shared.flushPendingSave()

            // Stop power observers BEFORE BackendService.stop(): a
            // willSleep notification during the SIGTERM window would call
            // VoicePipeline.disconnect() on a half-torn-down store.
            await MainActor.run { SystemPowerObserver.shared.stop() }

            await BodySpeechAdapter.shared.stop()
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
            if SetupWindowManager.shared.isOnboardingCompleted {
                OverlayWindowManager.shared.show()
            } else {
                SetupWindowManager.shared.show()
            }
        }
        return true
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        guard !self.isBodyAdapterOnly else { return }
        guard !self.isQuitting, !self.isStartingUp else { return }
        if SetupWindowManager.shared.isOnboardingCompleted {
            if OverlayWindowManager.shared.panel == nil {
                Task { @MainActor in self.showOverlay() }
            }
        } else {
            OverlayWindowManager.shared.close()
            SetupWindowManager.shared.show()
        }
    }

    @MainActor
    private func startApp() async {
        defer { self.isStartingUp = false }
        let isDev = ProcessInfo.processInfo.environment["BOBE_DEV"] != nil

        if DaemonConfig.endpoint.managesLocalProcess, !isDev {
            var serviceStarted = false
            var attempt = 0
            // Number of times the user clicked "Retry" after the first
            // round of 3 backend startup failures. After the second round
            // we surface a different copy that suggests reinstalling rather
            // than retrying forever.
            var retryRounds = 0
            while !serviceStarted {
                attempt += 1
                do {
                    try await BackendService.shared.start()
                    serviceStarted = true
                    // Clear the round counter on success — otherwise a later
                    // failure would render "keeps failing" copy on the first
                    // dialog of the new outage instead of "let's try again".
                    retryRounds = 0
                } catch {
                    logger.error("Backend start attempt \(attempt) failed: \(error.localizedDescription, privacy: .public)")
                    if attempt >= 3 {
                        retryRounds += 1
                        let shouldRetry = await showServiceErrorDialog(
                            error: error,
                            retryRounds: retryRounds
                        )
                        guard shouldRetry else {
                            NSApp.terminate(nil)
                            return
                        }
                        attempt = 0
                        continue
                    }
                }
            }
        } else if !DaemonConfig.endpoint.managesLocalProcess {
            logger.info("Remote daemon mode: skipping local process management")
            do {
                try await BackendService.shared.start()
            } catch {
                logger.error("Remote backend unavailable: \(error.localizedDescription, privacy: .public)")
                guard await self.showBackendErrorDialog(message: error.localizedDescription) else {
                    NSApp.terminate(nil)
                    return
                }
                await self.startApp()
                return
            }
        } else {
            logger.info("Dev mode: skipping service management (run `bobe serve` manually)")
        }

        if let warning = await BackendService.shared.startupWarning {
            self.store.surfaceWarning(warning)
        }
        if let bodyAdapter = BodyAdapterConfig.load() {
            await BodySpeechAdapter.shared.start(configuration: bodyAdapter)
        } else if ProcessInfo.processInfo.environment["BOBE_BODY_ADAPTER"] == "1" {
            logger.error(
                "BOBE_BODY_ADAPTER requires a loopback URL and BOBE_BODY__ADAPTER_TOKEN"
            )
        }
        if SetupWindowManager.shared.isOnboardingCompleted {
            self.showOverlay()
            self.store.connect()
            // Reconfirm voice models are still on disk after onboarding —
            // if the user wiped a cache or the install was partial, pop the
            // wizard so they can recover without hunting through Settings.
            Task { @MainActor in
                await self.repromptVoiceSetupIfModelsMissing()
            }
        } else {
            SetupWindowManager.shared.show()
            // store.connect() defers until handleWelcomeCompleted observer fires.
        }
    }

    /// Onboarding-completed user but a voice model went missing → re-pop the
    /// wizard so the install path is one click away. Skips silently when
    /// voice is disabled or both models are still present. Routes everything
    /// through `VoicePipeline.readiness` so the gate matches what the mic
    /// button sees.
    private func repromptVoiceSetupIfModelsMissing() async {
        let pipeline = VoicePipeline.shared
        // Order matters: pull settings first so activeSttLanguage is set,
        // then the presence probe routes to the right model bundle.
        await pipeline.refreshDaemonState()
        pipeline.bootstrapSttPresence()
        switch pipeline.readiness {
        case .modelsMissing, .failed:
            let readinessDesc = String(describing: pipeline.readiness)
            logger.warning(
                "voice models missing post-onboarding — re-opening voice setup (readiness=\(readinessDesc, privacy: .public))"
            )
            SetupWindowManager.shared.show(initialStep: .voiceSetup)
        case .ready, .preparing, .installing, .disabledByUser, .permissionMissing:
            return
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
    private func showBackendErrorDialog(message: String) async -> Bool {
        let alert = NSAlert()
        alert.messageText = L10n.tr("app.backend_error.title")
        alert.informativeText = L10n.tr("app.backend_error.message_format", message)
        alert.alertStyle = .critical
        alert.addButton(withTitle: L10n.tr("app.common.retry"))
        alert.addButton(withTitle: L10n.tr("app.common.quit"))
        return alert.runModal() == .alertFirstButtonReturn
    }

    @MainActor
    private func showServiceErrorDialog(error: Error, retryRounds: Int) async -> Bool {
        // Build a human summary first; relegate raw stderr to a disclosure-
        // style "details" footer that Expert users (or anyone curious) can
        // copy out. Most users only need the summary + a clear next action.
        let isPersistent = retryRounds >= 2
        let summary = isPersistent
            ? L10n.tr("app.service_error.persistent_summary")
            : L10n.tr("app.service_error.summary")

        var details = L10n.tr("app.service_error.detail_prefix")
            + error.localizedDescription
        if let stderr = await BackendService.shared.lastError {
            details += "\n\n" + L10n.tr("app.service_error.backend_output_prefix") + stderr
        }
        details += "\n\n" + L10n.tr("app.service_error.logs_hint")

        let alert = NSAlert()
        alert.messageText = L10n.tr("app.service_error.title")
        alert.informativeText = summary + "\n\n" + details
        alert.alertStyle = .critical
        // After the second round of retries we drop the optimistic "Retry"
        // CTA in favor of a clearer "Try again" that pairs with a "Quit"
        // suggesting reinstall via the logs hint above.
        alert.addButton(withTitle: L10n.tr("app.common.retry"))
        alert.addButton(withTitle: L10n.tr("app.common.quit"))
        return alert.runModal() == .alertFirstButtonReturn
    }
}

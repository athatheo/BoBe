import AppKit
import SwiftUI

/// Declarative replacement for the previous `TrayManager` + `NSStatusItem`
/// duo. SwiftUI's `MenuBarExtra` (macOS 13+) gives us:
///   • Automatic locale propagation (no `.id(localeOverride)` remount hack)
///   • Body re-eval on `@Observable` changes — no `menuWillOpen` rebuild
///   • A dynamic bar-icon recomputed from `store.stateType`, so the user
///     sees BoBe's mood at a glance without opening the menu
///   • No NSObject / NSMenuDelegate / @objc selector bookkeeping
///
/// **Trade-off acknowledged.** `MenuBarExtra(.menu)` style ignores
/// `Image()` views inside menu-item Buttons, so we can't put the state-
/// tinted dot leading the status text item the way the old NSStatusItem
/// did. We moved that affordance to the bar icon itself instead — which
/// is a UX upgrade (visible without clicking) rather than a downgrade.
///
/// **No third-party dependency.** We use only stock SwiftUI APIs — no
/// `MenuBarExtraAccess` or other community libraries. The native API
/// covers everything our tray actually needs: status text, action
/// buttons, nested locale submenu, keyboard shortcuts.
@MainActor
struct BobeMenuBarScene: Scene {
    /// Mirror the store so the menu body re-evaluates on state change.
    /// Reads are scoped to `store.stateType` + `store.isCapturing`; the
    /// rest of the context doesn't touch this scene's body.
    @State private var store = BobeStore.shared

    init(store: BobeStore = .shared) {
        self._store = State(initialValue: store)
    }

    var body: some Scene {
        MenuBarExtra(content: { self.menuContent }, label: { self.barIcon })
            // `.menu` style: native NSMenu chrome, every shortcut wired
            // to a Button gets a system-rendered key equivalent in the
            // menu, language submenu nests cleanly. Window style would
            // buy us nothing — we don't need sliders or rich UI here.
            .menuBarExtraStyle(.menu)
    }

    // MARK: - Bar icon

    /// Native face symbol plus a state-tinted activity dot. SF Symbols
    /// supplies the optical weight and alignment expected in the menu bar,
    /// while the dot preserves BoBe's at-a-glance activity signal.
    @ViewBuilder
    private var barIcon: some View {
        // Inactive states render with no overlay so the menu bar reads
        // "clean" at rest. Active states get the tinted dot. The dot
        // palette matches the old `TrayManager.color(for:)`.
        let dotColor = Self.barDotColor(for: self.store.stateType)
        ZStack(alignment: .bottomTrailing) {
            Image(systemName: "face.smiling")
                .symbolRenderingMode(.monochrome)
                .font(.system(size: 16, weight: .medium))
                .frame(width: 18, height: 18)
            if let dotColor {
                Circle()
                    .fill(dotColor)
                    .frame(width: 5, height: 5)
                    .overlay(
                        Circle().stroke(Color(.windowBackgroundColor), lineWidth: 1)
                    )
                    .offset(x: 2, y: 2)
            }
        }
        .accessibilityLabel(L10n.tr("tray.status.accessibility"))
    }

    private static func barDotColor(for state: BobeStateType) -> Color? {
        // Idle / loading / shutting-down → no dot (clean brand glyph).
        // Active → tinted accent. Mirrors TrayManager.color(for:).
        switch state {
        case .idle, .loading, .shuttingDown: nil
        case .capturing, .thinking, .speaking: Color(.systemGreen)
        case .wantsToSpeak: Color(.systemOrange)
        case .error: Color(.systemRed)
        }
    }

    // MARK: - Menu content

    @ViewBuilder
    private var menuContent: some View {
        // Status header — text-only because `.menu` style ignores
        // `Image()` in menu items. State is now visible on the bar icon
        // instead. Disabled button is the SwiftUI way to render a
        // non-interactive menu row.
        Button(self.statusLabel) {}
            .disabled(true)

        Divider()

        if SetupWindowManager.shared.isOnboardingCompleted {
            Button(self.overlayToggleLabel) {
                OverlayWindowManager.shared.toggle()
            }
            .keyboardShortcut("b", modifiers: [.command, .shift])

            Button(self.captureToggleLabel) {
                Task { _ = await self.store.toggleCapture() }
            }

            Button(L10n.tr("tray.settings")) {
                SettingsWindowManager.shared.show()
            }
            .keyboardShortcut(",", modifiers: .command)
        } else {
            Button(L10n.tr("app.setup_incomplete.retry")) {
                SetupWindowManager.shared.show()
            }
            .keyboardShortcut("b", modifiers: [.command, .shift])
        }

        // Nested submenu — locale picker. `Menu` works correctly in
        // `.menu` style. The localized labels follow the live override
        // because the L10n.tr lookup runs at body re-eval time.
        Menu(L10n.tr("tray.language")) {
            Button(L10n.tr("tray.language.system_default")) {
                self.store.updateLocale("")
            }
            Divider()
            ForEach(BobeStore.supportedLocales, id: \.self) { localeId in
                Button(Self.localeDisplayName(localeId)) {
                    self.store.updateLocale(localeId)
                }
            }
        }

        Button(L10n.tr("tray.check_updates")) {
            UpdaterManager.shared.checkForUpdates()
        }
        .disabled(!UpdaterManager.shared.canCheckForUpdates)

        Divider()

        Button(L10n.tr("tray.about")) {
            NSApp.activate()
            NSApp.orderFrontStandardAboutPanel(nil)
        }

        Button(L10n.tr("tray.quit")) {
            // Same shutdown path as before — `NSApp.terminate` flows
            // through `applicationShouldTerminate` which drains the
            // store, stops the backend, and tears down windows.
            self.store.disconnect()
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q", modifiers: .command)
    }

    // MARK: - Computed labels

    private var statusLabel: String {
        switch self.store.stateType {
        case .loading: L10n.tr("tray.state.connecting")
        case .idle: L10n.tr("tray.state.idle")
        case .capturing: L10n.tr("tray.state.capturing")
        case .thinking: L10n.tr("tray.state.thinking")
        case .speaking: L10n.tr("tray.state.speaking")
        case .wantsToSpeak: L10n.tr("tray.state.wants_to_speak")
        case .error: L10n.tr("tray.state.error")
        case .shuttingDown: L10n.tr("tray.state.shutting_down")
        }
    }

    private var overlayToggleLabel: String {
        OverlayWindowManager.shared.isVisible
            ? L10n.tr("tray.hide")
            : L10n.tr("tray.show")
    }

    private var captureToggleLabel: String {
        self.store.isCapturing
            ? L10n.tr("tray.capture.disable")
            : L10n.tr("tray.capture.enable")
    }

    private static func localeDisplayName(_ identifier: String) -> String {
        let locale = Locale(identifier: identifier)
        guard let native = locale.localizedString(forLanguageCode: identifier) else {
            return identifier
        }
        return native.prefix(1).uppercased() + native.dropFirst()
    }
}

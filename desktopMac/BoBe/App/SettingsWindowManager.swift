import AppKit
import SwiftUI

/// Manages the settings window (separate from overlay). Based on SettingsWindow.
@MainActor
final class SettingsWindowManager {
    static let shared = SettingsWindowManager()

    private var window: NSWindow?

    private init() {}

    func show() {
        if let window, window.isVisible {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        // Temporarily show in dock so settings can activate properly
        NSApp.setActivationPolicy(.regular)

        let settingsView = SettingsWindow()
            .environment(\.theme, ThemeStore.shared.currentTheme)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1050, height: 720),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "BoBe Settings"
        window.center()
        window.minSize = NSSize(width: 800, height: 550)
        window.contentView = NSHostingView(rootView: settingsView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window

        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.window = nil
                NSApp.setActivationPolicy(.accessory)
            }
        }
    }

    func close() {
        window?.close()
        window = nil
    }
}

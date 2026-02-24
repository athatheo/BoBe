import AppKit
import SwiftUI

/// Manages the settings window (separate from overlay). Matches Electron SettingsWindow.
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
    }

    func close() {
        window?.close()
        window = nil
    }
}

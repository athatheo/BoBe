import AppKit
import SwiftUI

/// Manages the settings window (separate from overlay).
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

        NSApp.setActivationPolicy(.regular)

        let theme = ThemeStore.shared.currentTheme
        let settingsView = SettingsWindow()

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1050, height: 720),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "BoBe Settings"
        window.center()
        window.minSize = NSSize(width: 800, height: 550)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .visible
        window.toolbar = nil
        window.backgroundColor = NSColor(theme.colors.background)
        window.contentView = NSHostingView(rootView: settingsView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window

        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.window = nil
                NSApp.setActivationPolicy(.accessory)
            }
        }
    }

    func close() {
        self.window?.close()
        self.window = nil
    }
}

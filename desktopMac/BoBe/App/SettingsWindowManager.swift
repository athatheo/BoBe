import AppKit
import SwiftUI

@MainActor
final class SettingsWindowManager: NSObject, NSWindowDelegate {
    static let shared = SettingsWindowManager()

    private var window: NSWindow?

    private override init() {}

    func show() {
        if let window {
            window.title = L10n.tr("settings.window.title")
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let theme = ThemeStore.shared.currentTheme
        let settingsView = SettingsWindow()

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1200, height: 820),
            styleMask: [.titled, .closable, .resizable, .miniaturizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = L10n.tr("settings.window.title")
        window.center()
        window.minSize = NSSize(width: 800, height: 550)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true
        window.animationBehavior = .none
        window.delegate = self
        window.backgroundColor = NSColor(theme.colors.background)
        window.contentViewController = NSHostingController(rootView: settingsView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }

    func close() {
        self.window?.orderOut(nil)
        self.window = nil
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        return false
    }

    func windowWillClose(_ notification: Notification) {
        guard let closingWindow = notification.object as? NSWindow, closingWindow == self.window else { return }
        self.window = nil
    }
}

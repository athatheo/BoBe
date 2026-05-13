import AppKit
import SwiftUI

@MainActor
final class SettingsWindowManager: NSObject, NSWindowDelegate {
    static let shared = SettingsWindowManager()

    private var window: NSWindow?

    private override init() {}

    func show(initialCategory: SettingsCategory? = nil) {
        if let window {
            window.title = L10n.tr("settings.window.title")
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            // Re-host the view so the initial category takes effect even when
            // the window already exists.
            if let initialCategory {
                window.contentViewController = NSHostingController(
                    rootView: SettingsWindow(initialCategory: initialCategory)
                )
            }
            return
        }

        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let width = min(max(screen.width * 0.72, 900), 1400)
        let height = min(max(screen.height * 0.78, 600), 1000)

        let window = BobeWindowFactory.make(
            contentRect: NSRect(x: 0, y: 0, width: width, height: height),
            styleMask: [.titled, .closable, .resizable, .miniaturizable, .fullSizeContentView],
            title: L10n.tr("settings.window.title"),
            rootView: SettingsWindow(initialCategory: initialCategory)
        )
        window.minSize = NSSize(width: 800, height: 550)
        window.delegate = self
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

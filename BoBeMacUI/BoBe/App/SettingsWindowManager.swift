import AppKit
import SwiftUI

@MainActor
final class SettingsWindowManager: NSObject, NSWindowDelegate {
    static let shared = SettingsWindowManager()

    private var window: NSWindow?

    override private init() {}

    func show(initialCategory: SettingsCategory? = nil) {
        if let window {
            window.title = L10n.tr("settings.window.title")
            window.makeKeyAndOrderFront(nil)
            NSApp.activate()
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
        // Honest initial floors so the window never opens too small to show
        // a panel's content (especially Engine and Goals, which are dense).
        // Bumped a second time after user feedback that 1020x680 still felt
        // cramped — descriptions and pickers were still colliding when
        // resized down.
        let width = min(max(screen.width * 0.72, 1200), 1500)
        let height = min(max(screen.height * 0.78, 800), 1100)

        let window = BobeWindowFactory.make(
            contentRect: NSRect(x: 0, y: 0, width: width, height: height),
            styleMask: [.titled, .closable, .resizable, .miniaturizable, .fullSizeContentView],
            title: L10n.tr("settings.window.title"),
            rootView: SettingsWindow(initialCategory: initialCategory)
        )
        // Floor picked so descriptions never wrap into pickers and the right
        // pane (Goals/Memories/Souls editors) still renders at full width.
        window.minSize = NSSize(width: 1100, height: 720)
        window.delegate = self
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
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

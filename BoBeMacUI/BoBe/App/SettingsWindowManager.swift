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
                SettingsEditCoordinator.shared.requestTransition {
                    let frame = window.frame
                    window.contentViewController = NSHostingController(
                        rootView: SettingsWindow(initialCategory: initialCategory)
                    )
                    window.setFrame(frame, display: true)
                }
            }
            return
        }

        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        // Honest initial floors so the window never opens too small to show
        // a panel's content (especially Engine and Goals, which are dense).
        // Bumped a second time after user feedback that 1020x680 still felt
        // cramped — descriptions and pickers were still colliding when
        // resized down.
        let initialSize = Self.initialSize(for: screen)

        let window = BobeWindowFactory.make(
            contentRect: NSRect(origin: .zero, size: initialSize),
            styleMask: [.titled, .closable, .resizable, .miniaturizable, .fullSizeContentView],
            title: L10n.tr("settings.window.title"),
            rootView: SettingsWindow(initialCategory: initialCategory)
        )
        // Use the productive desktop floor when the display allows it, while
        // still fitting smaller external displays instead of opening offscreen.
        window.minSize = Self.minimumSize(for: screen)
        window.delegate = self
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
        self.window = window
    }

    static func initialSize(for screen: NSRect) -> NSSize {
        let availableWidth = max(760, screen.width - 40)
        let availableHeight = max(520, screen.height - 40)
        return NSSize(
            width: min(max(screen.width * 0.72, min(1200, availableWidth)), 1500, availableWidth),
            height: min(max(screen.height * 0.78, min(800, availableHeight)), 1100, availableHeight)
        )
    }

    static func minimumSize(for screen: NSRect) -> NSSize {
        NSSize(
            width: min(1100, max(760, screen.width - 40)),
            height: min(720, max(520, screen.height - 40))
        )
    }

    func close() {
        SettingsEditCoordinator.shared.requestTransition { [weak self] in
            self?.window?.delegate = nil
            self?.window?.close()
            self?.window = nil
        }
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        SettingsEditCoordinator.shared.requestTransition {
            sender.orderOut(nil)
        }
        return false
    }

    func windowWillClose(_ notification: Notification) {
        guard let closingWindow = notification.object as? NSWindow, closingWindow == self.window else { return }
        self.window = nil
    }
}

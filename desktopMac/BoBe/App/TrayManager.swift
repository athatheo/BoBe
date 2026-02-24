import AppKit
import SwiftUI
import Observation

/// System tray (menu bar) manager. Based on TrayManager.
@MainActor
final class TrayManager {
    static let shared = TrayManager()

    private var statusItem: NSStatusItem?
    private var store = BobeStore.shared

    private init() {}

    func setup() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        if let button = statusItem?.button {
            // Use template icon from resources (macOS auto-handles dark/light)
            if let trayIcon = loadTrayIcon() {
                trayIcon.isTemplate = true
                trayIcon.size = NSSize(width: 18, height: 18)
                button.image = trayIcon
            } else {
                button.image = NSImage(systemSymbolName: "brain.head.profile", accessibilityDescription: "BoBe")
                button.image?.size = NSSize(width: 18, height: 18)
                button.image?.isTemplate = true
            }
        }

        updateMenu()
    }

    func updateMenu() {
        let menu = NSMenu()

        // Status text
        let statusText: String
        switch store.stateType {
        case .loading: statusText = "Connecting..."
        case .idle: statusText = "Idle"
        case .capturing: statusText = "Looking..."
        case .thinking: statusText = "Thinking..."
        case .speaking: statusText = "Speaking"
        case .wantsToSpeak: statusText = "Has something to say"
        case .error: statusText = "Error"
        }
        let statusItem = NSMenuItem(title: "Status: \(statusText)", action: nil, keyEquivalent: "")
        statusItem.isEnabled = false
        menu.addItem(statusItem)

        menu.addItem(.separator())

        // Capture toggle
        let captureTitle = store.isCapturing ? "Stop Looking" : "Allow Looking"
        let captureItem = NSMenuItem(title: captureTitle, action: #selector(toggleCapture), keyEquivalent: "l")
        captureItem.target = self
        menu.addItem(captureItem)

        menu.addItem(.separator())

        // Show/Hide
        let overlayVisible = OverlayWindowManager.shared.isVisible
        let showHideTitle = overlayVisible ? "Hide BoBe" : "Show BoBe"
        let showHideItem = NSMenuItem(title: showHideTitle, action: #selector(toggleOverlay), keyEquivalent: "b")
        showHideItem.target = self
        menu.addItem(showHideItem)

        // Settings
        let settingsItem = NSMenuItem(title: "Settings...", action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        menu.addItem(.separator())

        // Quit
        let quitItem = NSMenuItem(title: "Quit BoBe", action: #selector(quitApp), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        self.statusItem?.menu = menu
    }

    @objc private func toggleCapture() {
        Task {
            _ = await store.toggleCapture()
            updateMenu()
        }
    }

    @objc private func toggleOverlay() {
        let manager = OverlayWindowManager.shared
        if manager.isVisible {
            manager.hide()
        } else {
            manager.show()
        }
        updateMenu()
    }

    @objc private func openSettings() {
        SettingsWindowManager.shared.show()
    }

    @objc private func quitApp() {
        store.disconnect()
        NSApplication.shared.terminate(nil)
    }

    private func loadTrayIcon() -> NSImage? {
        // Look for trayIconTemplate@2x.png first (retina), then 1x
        for name in ["trayIconTemplate@2x", "trayIconTemplate"] {
            if let path = Bundle.main.path(forResource: name, ofType: "png"),
               let image = NSImage(contentsOfFile: path) {
                return image
            }
        }
        // Dev mode: try loading from source tree
        let srcPath = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Resources/trayIconTemplate@2x.png")
        if let image = NSImage(contentsOf: srcPath) {
            return image
        }
        return nil
    }
}

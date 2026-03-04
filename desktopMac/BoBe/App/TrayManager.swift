import AppKit
import Observation
import SwiftUI

/// System tray (menu bar) manager.
@MainActor
final class TrayManager: NSObject, NSMenuDelegate {
    private var statusItem: NSStatusItem?
    private let store: BobeStore

    init(store: BobeStore) {
        self.store = store
        super.init()
    }

    func setup() {
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        if let button = statusItem?.button {
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

        self.updateMenu()
    }

    func updateMenu() {
        let menu = NSMenu()

        let statusText = switch self.store.stateType {
        case .loading: "Connecting..."
        case .idle: "Idle"
        case .capturing: "Looking..."
        case .thinking: "Thinking..."
        case .speaking: "Speaking"
        case .wantsToSpeak: "Has something to say"
        case .error: "Error"
        case .shuttingDown: "Shutting down..."
        }
        let statusItem = NSMenuItem(title: "Status: \(statusText)", action: nil, keyEquivalent: "")
        statusItem.isEnabled = false
        menu.addItem(statusItem)

        menu.addItem(.separator())

        let overlayVisible = OverlayWindowManager.shared.isVisible
        let showHideTitle = overlayVisible ? "Hide BoBe" : "Show BoBe"
        let showHideItem = NSMenuItem(title: showHideTitle, action: #selector(toggleOverlay), keyEquivalent: "b")
        showHideItem.target = self
        menu.addItem(showHideItem)

        let captureTitle = self.store.isCapturing ? "Stop Capture" : "Start Capture"
        let captureItem = NSMenuItem(title: captureTitle, action: #selector(toggleCapture), keyEquivalent: "")
        captureItem.target = self
        captureItem.state = self.store.isCapturing ? .on : .off
        menu.addItem(captureItem)

        let settingsItem = NSMenuItem(title: "Settings...", action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        let checkUpdatesItem = NSMenuItem(
            title: "Check for Updates...",
            action: #selector(checkForUpdates),
            keyEquivalent: ""
        )
        checkUpdatesItem.target = self
        checkUpdatesItem.isEnabled = UpdaterManager.shared.canCheckForUpdates
        menu.addItem(checkUpdatesItem)

        menu.addItem(.separator())

        let aboutItem = NSMenuItem(
            title: "About BoBe",
            action: #selector(showAbout),
            keyEquivalent: ""
        )
        aboutItem.target = self
        menu.addItem(aboutItem)

        let quitItem = NSMenuItem(title: "Quit BoBe", action: #selector(quitApp), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        self.statusItem?.menu = menu
        menu.delegate = self
    }

    /// Rebuild menu every time it opens — always shows fresh state
    nonisolated func menuWillOpen(_ menu: NSMenu) {
        Task { @MainActor in
            self.updateMenu()
        }
    }

    @objc
    private func toggleOverlay() {
        let manager = OverlayWindowManager.shared
        if manager.isVisible {
            manager.hide()
        } else {
            manager.show()
        }
        self.updateMenu()
    }

    @objc
    private func toggleCapture() {
        Task {
            _ = await self.store.toggleCapture()
            self.updateMenu()
        }
    }

    @objc
    private func openSettings() {
        SettingsWindowManager.shared.show()
    }

    @objc
    private func checkForUpdates() {
        UpdaterManager.shared.checkForUpdates()
    }

    @objc
    private func showAbout() {
        NSApp.activate(ignoringOtherApps: true)
        NSApp.orderFrontStandardAboutPanel(nil)
    }

    @objc
    private func quitApp() {
        self.store.disconnect()
        NSApplication.shared.terminate(nil)
    }

    private func loadTrayIcon() -> NSImage? {
        for name in ["trayIconTemplate@2x", "trayIconTemplate"] {
            if let url = Bundle.appResources.url(forResource: name, withExtension: "png"),
               let image = NSImage(contentsOf: url) {
                return image
            }
        }
        let srcPath = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Resources/trayIconTemplate@2x.png")
        if let image = NSImage(contentsOf: srcPath) {
            return image
        }
        return nil
    }
}

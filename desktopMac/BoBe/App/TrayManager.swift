import AppKit
import Observation
import os
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "TrayManager")

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
                button.image = NSImage(
                    systemSymbolName: "brain.head.profile",
                    accessibilityDescription: L10n.tr("app.brand_name")
                )
                button.image?.size = NSSize(width: 18, height: 18)
                button.image?.isTemplate = true
            }
        }

        self.updateMenu()
    }

    func updateMenu() {
        let menu = NSMenu()

        let statusText = switch self.store.stateType {
        case .loading: L10n.tr("tray.state.connecting")
        case .idle: L10n.tr("tray.state.idle")
        case .capturing: L10n.tr("tray.state.capturing")
        case .thinking: L10n.tr("tray.state.thinking")
        case .speaking: L10n.tr("tray.state.speaking")
        case .wantsToSpeak: L10n.tr("tray.state.wants_to_speak")
        case .error: L10n.tr("tray.state.error")
        case .shuttingDown: L10n.tr("tray.state.shutting_down")
        }
        let statusItem = NSMenuItem(
            title: L10n.tr("tray.status_format", statusText),
            action: nil,
            keyEquivalent: ""
        )
        statusItem.isEnabled = false
        menu.addItem(statusItem)

        menu.addItem(.separator())

        let overlayVisible = OverlayWindowManager.shared.isVisible
        let showHideTitle = overlayVisible ? L10n.tr("tray.hide") : L10n.tr("tray.show")
        let showHideItem = NSMenuItem(title: showHideTitle, action: #selector(toggleOverlay), keyEquivalent: "b")
        showHideItem.target = self
        menu.addItem(showHideItem)

        let captureTitle = self.store.isCapturing ? L10n.tr("tray.capture.stop") : L10n.tr("tray.capture.start")
        let captureItem = NSMenuItem(title: captureTitle, action: #selector(toggleCapture), keyEquivalent: "")
        captureItem.target = self
        captureItem.state = self.store.isCapturing ? .on : .off
        menu.addItem(captureItem)

        let settingsItem = NSMenuItem(title: L10n.tr("tray.settings"), action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        let languageItem = NSMenuItem(title: L10n.tr("tray.language"), action: nil, keyEquivalent: "")
        languageItem.submenu = buildLanguageSubmenu()
        menu.addItem(languageItem)

        let checkUpdatesItem = NSMenuItem(
            title: L10n.tr("tray.check_updates"),
            action: #selector(checkForUpdates),
            keyEquivalent: ""
        )
        checkUpdatesItem.target = self
        checkUpdatesItem.isEnabled = UpdaterManager.shared.canCheckForUpdates
        menu.addItem(checkUpdatesItem)

        menu.addItem(.separator())

        let aboutItem = NSMenuItem(
            title: L10n.tr("tray.about"),
            action: #selector(showAbout),
            keyEquivalent: ""
        )
        aboutItem.target = self
        menu.addItem(aboutItem)

        let quitItem = NSMenuItem(title: L10n.tr("tray.quit"), action: #selector(quitApp), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        self.statusItem?.menu = menu
        menu.delegate = self
    }

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

    private func buildLanguageSubmenu() -> NSMenu {
        let submenu = NSMenu()

        let systemDefault = NSMenuItem(
            title: L10n.tr("tray.language.system_default"),
            action: #selector(changeLanguage(_:)),
            keyEquivalent: ""
        )
        systemDefault.target = self
        systemDefault.representedObject = "" as String
        systemDefault.state = self.store.localeOverride.isEmpty ? .on : .off
        submenu.addItem(systemDefault)

        submenu.addItem(.separator())

        for localeId in self.store.supportedLocales {
            let locale = Locale(identifier: localeId)
            let nativeName = locale.localizedString(forLanguageCode: localeId)?
                .prefix(1).uppercased()
                .appending(String(locale.localizedString(forLanguageCode: localeId)?.dropFirst() ?? ""))
                ?? localeId
            let item = NSMenuItem(title: nativeName, action: #selector(changeLanguage(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = localeId
            item.state = self.store.localeOverride == localeId ? .on : .off
            submenu.addItem(item)
        }

        return submenu
    }

    @objc
    private func changeLanguage(_ sender: NSMenuItem) {
        let localeId = sender.representedObject as? String ?? ""
        let previousLocale = store.localeOverride
        Task { @MainActor in
            self.store.updateLocale(localeId)
            do {
                var req = SettingsUpdateRequest()
                req.localeOverride = localeId
                _ = try await DaemonClient.shared.updateSettings(req)
            } catch {
                logger.error("Failed to persist language change: \(error.localizedDescription)")
                self.store.updateLocale(previousLocale)
            }
            self.updateMenu()
        }
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

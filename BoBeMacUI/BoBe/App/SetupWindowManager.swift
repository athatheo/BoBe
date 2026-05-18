import AppKit
import SwiftUI

extension Notification.Name {
    static let bobeWelcomeCompleted = Notification.Name("bobe.welcomeCompleted")
}

@MainActor
final class SetupWindowManager: NSObject, NSWindowDelegate {
    static let shared = SetupWindowManager()

    static let onboardingCompletedKey = "bobe.onboarding_completed"

    private var window: NSWindow?

    override private init() {}

    var isOnboardingCompleted: Bool {
        UserDefaults.standard.bool(forKey: Self.onboardingCompletedKey)
    }

    func markOnboardingCompleted() {
        UserDefaults.standard.set(true, forKey: Self.onboardingCompletedKey)
    }

    func show() {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }

        let view = WelcomeWizard(
            onComplete: { [weak self] in
                self?.markOnboardingCompleted()
                NotificationCenter.default.post(name: .bobeWelcomeCompleted, object: nil)
            }
        )

        let window = BobeWindowFactory.make(
            contentRect: NSRect(x: 0, y: 0, width: 540, height: 640),
            styleMask: [.titled, .closable, .fullSizeContentView],
            title: L10n.tr("setup.window.title"),
            animationBehavior: .documentWindow,
            rootView: view
        )
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
        guard let closing = notification.object as? NSWindow, closing == self.window else { return }
        self.window = nil
    }
}

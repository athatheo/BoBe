import AppKit
import SwiftUI

extension Notification.Name {
    static let bobeWelcomeCompleted = Notification.Name("bobe.welcomeCompleted")
}

@MainActor
final class SetupWindowManager: NSObject, NSWindowDelegate {
    static let shared = SetupWindowManager()

    static let onboardingCompletedKey = "bobe.onboarding_completed"
    private static let openChatAfterOnboardingKey = "bobe.open_chat_after_onboarding"

    private var window: NSWindow?

    override private init() {}

    var isOnboardingCompleted: Bool {
        UserDefaults.standard.bool(forKey: Self.onboardingCompletedKey)
    }

    func markOnboardingCompleted() {
        UserDefaults.standard.set(true, forKey: Self.onboardingCompletedKey)
        UserDefaults.standard.set(true, forKey: Self.openChatAfterOnboardingKey)
    }

    func consumeOpenChatAfterOnboarding() -> Bool {
        let defaults = UserDefaults.standard
        guard defaults.bool(forKey: Self.openChatAfterOnboardingKey) else { return false }
        defaults.removeObject(forKey: Self.openChatAfterOnboardingKey)
        return true
    }

    func show(initialStep: WelcomeStep? = nil) {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }

        let view = WelcomeWizard(
            onComplete: { [weak self] in
                self?.markOnboardingCompleted()
                NotificationCenter.default.post(name: .bobeWelcomeCompleted, object: nil)
            },
            initialStep: initialStep
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
        // Closing a document-animated NSWindow while its SwiftUI completion
        // button is still unwinding can race AppKit's transform animation
        // teardown. Hide first and detach the hosting tree; AppKit releases
        // the now-empty window safely after the current event transaction.
        self.window?.orderOut(nil)
        self.window?.contentViewController = nil
        self.window?.delegate = nil
        self.window = nil
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        guard !self.isOnboardingCompleted else { return true }

        let alert = NSAlert()
        alert.messageText = L10n.tr("app.setup_incomplete.title")
        alert.informativeText = L10n.tr("app.setup_incomplete.message")
        alert.alertStyle = .warning
        alert.addButton(withTitle: L10n.tr("app.setup_incomplete.retry"))
        alert.addButton(withTitle: L10n.tr("app.common.quit"))
        if alert.runModal() == .alertSecondButtonReturn {
            NSApp.terminate(nil)
        } else {
            sender.makeKeyAndOrderFront(nil)
            NSApp.activate()
        }
        return false
    }

    func windowWillClose(_ notification: Notification) {
        guard let closing = notification.object as? NSWindow, closing == self.window else { return }
        self.window = nil
    }
}

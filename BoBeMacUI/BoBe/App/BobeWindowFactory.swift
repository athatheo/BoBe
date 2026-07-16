import AppKit
import SwiftUI

/// Shared NSWindow factory for Bobe's chrome (Settings + Setup). Both windows
/// share the same titlebar treatment, theme background, hosting-controller
/// pattern, and movability — extracted here to keep the two window managers
/// from drifting.
@MainActor
enum BobeWindowFactory {
    /// Build an NSWindow with Bobe's standard titlebar/background/hosting
    /// treatment. The caller still owns the window lifecycle (delegate
    /// assignment, ordering front, activation).
    static func make(
        contentRect: NSRect,
        styleMask: NSWindow.StyleMask,
        title: String,
        animationBehavior: NSWindow.AnimationBehavior = .none,
        rootView: some View
    ) -> NSWindow {
        let theme = ThemeStore.shared.currentTheme

        let window = NSWindow(
            contentRect: contentRect,
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.isRestorable = false
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true
        window.animationBehavior = animationBehavior
        window.backgroundColor = NSColor(theme.colors.background)
        window.contentViewController = NSHostingController(rootView: rootView)
        window.setContentSize(contentRect.size)
        window.center()
        return window
    }
}

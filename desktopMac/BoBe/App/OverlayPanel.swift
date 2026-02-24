import AppKit

/// Transparent, frameless, always-on-top floating panel for the BoBe overlay.
/// Equivalent to Electron's BrowserWindow with transparent: true, frame: false, alwaysOnTop: true.
final class OverlayPanel: NSPanel {
    init(contentRect: NSRect) {
        super.init(
            contentRect: contentRect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .floating
        isMovableByWindowBackground = true
        hidesOnDeactivate = false
        isReleasedWhenClosed = false

        collectionBehavior = [
            .canJoinAllSpaces,
            .fullScreenAuxiliary,
            .stationary
        ]

        // Ignore mouse events on transparent areas
        ignoresMouseEvents = false
    }

    // Allow the panel to become key for text input
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

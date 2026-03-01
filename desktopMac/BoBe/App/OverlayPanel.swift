import AppKit

/// Content view that passes clicks through transparent areas — game HUD behavior.
/// Only accepts hits on actual child views; empty space falls through to apps beneath.
final class PassthroughContentView: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        let local = convert(point, from: superview)
        guard bounds.contains(local) else { return nil }
        for child in subviews.reversed() {
            let childPoint = child.convert(local, from: self)
            if let hit = child.hitTest(childPoint) {
                return hit
            }
        }
        return nil
    }
}

/// Transparent, frameless, always-on-top floating panel for the BoBe overlay.
final class OverlayPanel: NSPanel {
    init(contentRect: NSRect) {
        super.init(
            contentRect: contentRect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        let passthrough = PassthroughContentView(frame: NSRect(origin: .zero, size: contentRect.size))
        passthrough.autoresizingMask = [.width, .height]
        contentView = passthrough

        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .floating
        isMovable = true
        isMovableByWindowBackground = true
        hidesOnDeactivate = false
        isReleasedWhenClosed = false

        collectionBehavior = [
            .canJoinAllSpaces,
            .fullScreenAuxiliary,
        ]

        ignoresMouseEvents = false
    }

    /// Allow the panel to become key for text input
    override var canBecomeKey: Bool {
        true
    }

    override var canBecomeMain: Bool {
        false
    }
}

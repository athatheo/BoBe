import AppKit
import SwiftUI

/// Manages the overlay panel's position and sizing.
/// Anchors the panel to the bottom-right of the screen, matching the original behavior.
@MainActor
final class OverlayWindowManager {
    static let shared = OverlayWindowManager()

    private(set) var panel: OverlayPanel?

    private init() {}

    func createPanel(with rootView: some View) {
        // Close existing panel to prevent orphans
        if panel != nil { close() }

        let screen = NSScreen.main ?? NSScreen.screens[0]
        let screenFrame = screen.visibleFrame
        let initialWidth = WindowSizes.widthCollapsed
        let initialHeight = WindowSizes.heightCollapsed

        let origin = NSPoint(
            x: screenFrame.maxX - initialWidth - WindowSizes.margin,
            y: screenFrame.minY + WindowSizes.margin
        )
        let rect = NSRect(origin: origin, size: NSSize(width: initialWidth, height: initialHeight))

        let panel = OverlayPanel(contentRect: rect)

        let hostView = NSHostingView(rootView: rootView)
        hostView.frame = panel.contentView!.bounds
        hostView.autoresizingMask = [.width, .height]
        panel.contentView?.addSubview(hostView)

        panel.orderFrontRegardless()
        self.panel = panel
    }

    /// Resize the panel anchored to bottom-right
    func resize(width: CGFloat, height: CGFloat) {
        guard let panel, let screen = panel.screen ?? NSScreen.main else { return }
        let screenFrame = screen.visibleFrame

        let clampedWidth = min(width, screenFrame.width - WindowSizes.margin * 2)
        let clampedHeight = min(height, screenFrame.height - WindowSizes.margin * 2)

        let newX = screenFrame.maxX - clampedWidth - WindowSizes.margin
        let newY = screenFrame.minY + WindowSizes.margin

        let newFrame = NSRect(
            x: newX, y: newY,
            width: clampedWidth, height: clampedHeight
        )
        panel.setFrame(newFrame, display: true, animate: false)
    }

    func show() {
        panel?.orderFrontRegardless()
    }

    func hide() {
        panel?.orderOut(nil)
    }

    var isVisible: Bool {
        panel?.isVisible ?? false
    }

    func close() {
        panel?.close()
        panel = nil
    }
}

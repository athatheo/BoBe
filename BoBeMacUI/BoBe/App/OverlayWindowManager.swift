import AppKit
import SwiftUI

private final class OverlayHostingView<Content: View>: NSHostingView<Content> {
    override var mouseDownCanMoveWindow: Bool {
        true
    }
}

@MainActor
final class OverlayWindowManager {
    static let shared = OverlayWindowManager()

    private(set) var panel: OverlayPanel?

    private init() {}

    func createPanel(with rootView: some View) {
        if panel != nil { self.close() }

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
        panel.contentView?.wantsLayer = true
        panel.contentView?.layer?.masksToBounds = false

        let hostView = OverlayHostingView(rootView: rootView)
        if let contentView = panel.contentView {
            hostView.frame = contentView.bounds
        }
        hostView.autoresizingMask = [.width, .height]
        hostView.wantsLayer = true
        hostView.layer?.masksToBounds = false
        panel.contentView?.addSubview(hostView)

        panel.orderFrontRegardless()
        self.panel = panel
    }

    func resize(width: CGFloat, height: CGFloat) {
        guard let panel, let screen = panel.screen ?? NSScreen.main else { return }
        let screenFrame = screen.visibleFrame

        let maxWidth = screenFrame.width - WindowSizes.margin * 2
        let maxHeight = screenFrame.height - WindowSizes.margin * 2
        let clampedWidth = min(max(width, WindowSizes.widthCollapsed), maxWidth)
        let clampedHeight = min(max(height, WindowSizes.heightCollapsed), maxHeight)

        let currentFrame = panel.frame
        var newX = currentFrame.maxX - clampedWidth
        var newY = currentFrame.minY
        newX = min(
            max(newX, screenFrame.minX + WindowSizes.margin),
            screenFrame.maxX - clampedWidth - WindowSizes.margin
        )
        newY = min(
            max(newY, screenFrame.minY + WindowSizes.margin),
            screenFrame.maxY - clampedHeight - WindowSizes.margin
        )

        let newFrame = NSRect(
            x: newX, y: newY,
            width: clampedWidth, height: clampedHeight
        )
        panel.setFrame(newFrame, display: true, animate: false)
    }

    func show() {
        self.panel?.orderFrontRegardless()
    }

    func hide() {
        self.panel?.orderOut(nil)
    }

    var isVisible: Bool {
        self.panel?.isVisible ?? false
    }

    func close() {
        self.panel?.close()
        self.panel = nil
    }
}

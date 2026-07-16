import AppKit
import SwiftUI

private final class OverlayHostingView<Content: View>: NSHostingView<Content> {
    override var mouseDownCanMoveWindow: Bool {
        true
    }
}

@MainActor
final class OverlayWindowManager: NSObject, NSWindowDelegate {
    static let shared = OverlayWindowManager()

    private(set) var panel: OverlayPanel?

    private static let anchorXKey = "bobe.overlay.anchor_x"
    private static let anchorYKey = "bobe.overlay.anchor_y"

    override private init() {}

    func createPanel(with rootView: some View) {
        if panel != nil {
            self.close()
        }

        let savedAnchor = self.savedAnchor
        let screen = savedAnchor.flatMap { anchor in
            NSScreen.screens.first { $0.visibleFrame.insetBy(dx: -40, dy: -40).contains(anchor) }
        } ?? NSScreen.main ?? NSScreen.screens[0]
        let screenFrame = screen.visibleFrame
        let initialWidth = WindowSizes.widthCollapsed
        let initialHeight = WindowSizes.heightCollapsed

        let defaultAnchor = NSPoint(
            x: screenFrame.maxX - WindowSizes.margin,
            y: screenFrame.minY + WindowSizes.margin
        )
        let anchor = savedAnchor ?? defaultAnchor
        let origin = Self.clampedOrigin(
            for: anchor,
            screenFrame: screenFrame,
            size: NSSize(width: initialWidth, height: initialHeight)
        )
        let rect = NSRect(origin: origin, size: NSSize(width: initialWidth, height: initialHeight))

        let panel = OverlayPanel(contentRect: rect)
        panel.delegate = self
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

    /// Show if hidden, hide if visible. Used by the main-menu Toggle Overlay
    /// command (Cmd+Shift+B) and the tray Show/Hide item.
    func toggle() {
        if self.isVisible {
            self.hide()
        } else {
            self.show()
        }
    }

    func close() {
        self.panel?.close()
        self.panel = nil
    }

    func windowDidMove(_ notification: Notification) {
        guard let movedPanel = notification.object as? OverlayPanel,
              movedPanel == self.panel
        else { return }
        self.saveAnchor(for: movedPanel)
    }

    private var savedAnchor: NSPoint? {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: Self.anchorXKey) != nil,
              defaults.object(forKey: Self.anchorYKey) != nil
        else { return nil }
        return NSPoint(
            x: defaults.double(forKey: Self.anchorXKey),
            y: defaults.double(forKey: Self.anchorYKey)
        )
    }

    private func saveAnchor(for panel: NSPanel) {
        let defaults = UserDefaults.standard
        defaults.set(panel.frame.maxX, forKey: Self.anchorXKey)
        defaults.set(panel.frame.minY, forKey: Self.anchorYKey)
    }

    static func clampedOrigin(
        for anchor: NSPoint,
        screenFrame: NSRect,
        size: NSSize
    ) -> NSPoint {
        NSPoint(
            x: min(
                max(anchor.x - size.width, screenFrame.minX + WindowSizes.margin),
                screenFrame.maxX - size.width - WindowSizes.margin
            ),
            y: min(
                max(anchor.y, screenFrame.minY + WindowSizes.margin),
                screenFrame.maxY - size.height - WindowSizes.margin
            )
        )
    }
}

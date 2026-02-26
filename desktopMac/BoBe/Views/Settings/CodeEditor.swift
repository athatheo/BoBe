import AppKit
import SwiftUI

/// Monaco-like plain text editor backed by NSTextView for macOS settings panes.
struct CodeEditor: NSViewRepresentable {
    @Binding var text: String
    let theme: ThemeConfig
    var fontSize: CGFloat = 13

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false

        let textView = NSTextView()
        textView.delegate = context.coordinator
        textView.string = text
        textView.isEditable = true
        textView.isSelectable = true
        textView.isRichText = false
        textView.importsGraphics = false
        textView.usesFindBar = true
        textView.allowsUndo = true
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.textContainerInset = NSSize(width: 6, height: 8)
        textView.isHorizontallyResizable = false
        textView.isVerticallyResizable = true
        textView.autoresizingMask = [.width]
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
        applyTheme(to: textView)
        textView.font = .monospacedSystemFont(ofSize: fontSize, weight: .regular)

        scrollView.documentView = textView
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let textView = scrollView.documentView as? NSTextView else { return }
        if textView.string != text {
            textView.string = text
        }
        textView.font = .monospacedSystemFont(ofSize: fontSize, weight: .regular)
        applyTheme(to: textView)
    }

    private func applyTheme(to textView: NSTextView) {
        textView.backgroundColor = NSColor(theme.colors.surface)
        textView.textColor = NSColor(theme.colors.text)
        textView.insertionPointColor = NSColor(theme.colors.primary)
        textView.selectedTextAttributes = [
            .backgroundColor: NSColor(theme.colors.primary).withAlphaComponent(0.25),
            .foregroundColor: NSColor(theme.colors.text)
        ]
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: CodeEditor

        init(_ parent: CodeEditor) {
            self.parent = parent
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            let value = textView.string
            if parent.text != value {
                parent.text = value
            }
        }
    }
}

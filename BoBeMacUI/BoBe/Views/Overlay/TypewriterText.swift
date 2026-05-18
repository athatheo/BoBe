import SwiftUI

/// Drop-in `Text` replacement that types characters in over time and
/// trails an optional blinking cursor while typing.
///
/// Two modes:
/// - **restart mode** (default): every change to `text` restarts typing
///   from the empty string. Use for short, complete-on-arrival labels.
/// - **append mode** (`appendMode: true`): when the new `text` extends
///   the previously-displayed prefix, typing continues from where it
///   left off. Use for streaming BoBe message bodies where the content
///   grows token-by-token over many seconds — without this the visible
///   text would flash back to empty on every chunk, looking broken.
///
/// Lifecycle: the typing loop and the cursor blink both live on `.task(id:)`
/// bound to a stable identity (the text content for typing, a Reduce Motion
/// gate for the cursor). This is what lets callers swap the surrounding
/// view's appearance freely without forcing a `.id(...)` remount on the
/// parent — TypewriterText handles its own reset cleanly.
struct TypewriterText: View {
    let text: String
    let cursorColor: Color
    var charDelay: TimeInterval = 0.035
    var appendMode: Bool = false
    var showCursor: Bool = true

    @State private var displayed = ""
    @State private var isTyping = false
    @State private var cursorVisible = true

    var body: some View {
        HStack(alignment: .bottom, spacing: 1) {
            Text(self.displayed)
            if self.showCursor, self.isTyping || !self.displayed.isEmpty {
                Text("|")
                    .foregroundStyle(self.cursorColor)
                    .opacity(self.cursorVisible ? 1 : 0)
            }
        }
        .multilineTextAlignment(.leading)
        .fixedSize(horizontal: false, vertical: true)
        // Typing loop. `.task(id:)` cancels + restarts whenever `text`
        // changes, so callers don't need to mount this view via `.id()`
        // to force a restart — the typing state self-resets cleanly.
        .task(id: self.text) {
            await self.runTyping(to: self.text)
        }
        // Cursor blink. Lives separately so it's not cancelled by text
        // updates in append mode. Reduce Motion + `showCursor == false`
        // both short-circuit the loop entirely so no main-actor work runs
        // when the cursor isn't visible anyway.
        .task(id: self.showCursor && OverlayMotionRuntime.shouldAnimate) {
            guard self.showCursor, OverlayMotionRuntime.shouldAnimate else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(400))
                guard !Task.isCancelled else { return }
                self.cursorVisible.toggle()
            }
        }
    }

    /// Decide whether to restart from empty or continue from where we left
    /// off, then type the new tail one character at a time. The whole loop
    /// is scoped to `.task(id: text)` so cancellation comes free with the
    /// next text change.
    private func runTyping(to target: String) async {
        // Append mode: if the new target extends the currently-displayed
        // prefix, keep what we've typed and type only the tail. Otherwise
        // reset and type the whole new target.
        let canContinue = self.appendMode && target.hasPrefix(self.displayed)
        if !canContinue {
            self.displayed = ""
        }
        let remaining = String(target[self.displayed.endIndex...])
        if remaining.isEmpty {
            self.isTyping = false
            return
        }
        self.isTyping = true
        for character in remaining {
            if Task.isCancelled { return }
            self.displayed.append(character)
            try? await Task.sleep(for: .seconds(self.charDelay))
        }
        self.isTyping = false
    }
}

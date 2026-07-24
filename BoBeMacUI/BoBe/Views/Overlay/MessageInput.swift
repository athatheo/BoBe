import SwiftUI

struct MessageInput: View {
    @Binding var text: String
    let onSend: (String) -> Bool
    let onClose: () -> Void
    var showsHistory = false
    var canShowHistory = false
    var onToggleHistory: () -> Void = {}
    var feedbackMessage: String?
    var isBusy = false

    @FocusState private var isFocused: Bool
    @Environment(\.theme) private var theme
    @State private var isCloseHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            if let feedbackMessage {
                HStack(spacing: 6) {
                    Image(systemName: self.isBusy ? "hourglass" : "info.circle")
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(self.theme.colors.primary)

                    Text(feedbackMessage)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(2)

                    Spacer(minLength: 0)
                }
                .transition(.opacity)
            }

            HStack(alignment: .center, spacing: 6) {
                if self.canShowHistory {
                    self.historyButton
                }

                ZStack(alignment: .leading) {
                    if self.text.isEmpty {
                        Text(L10n.tr("overlay.input.placeholder.hint.notice"))
                            .accessibilityHidden(true)
                            .bobeTextStyle(.inputField)
                            .foregroundStyle(self.placeholderColor)
                            .allowsHitTesting(false)
                    }

                    TextField("", text: self.$text, axis: .vertical)
                        .accessibilityLabel(L10n.tr("overlay.input.composer.accessibility"))
                        .accessibilityIdentifier("overlay.composer.field")
                        .textFieldStyle(.plain)
                        .bobeTextStyle(.inputField)
                        .lineSpacing(2)
                        .foregroundStyle(self.inputTextColor)
                        .tint(self.theme.colors.primary)
                        .lineLimit(1 ... 3)
                        .focused(self.$isFocused)
                        .onSubmit { self.handleSubmit() }
                        .onKeyPress(.escape) {
                            self.onClose()
                            return .handled
                        }
                }
                .padding(.horizontal, 4)
                .frame(maxWidth: .infinity, alignment: .leading)

                if !self.isBusy {
                    self.sendButton
                }

                self.closeButton
            }
        }
        .padding(.horizontal, 9)
        .padding(.vertical, 7)
        .background(self.theme.colors.surface)
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(self.theme.colors.border.opacity(0.85), lineWidth: 1)
        )
        .contentShape(RoundedRectangle(cornerRadius: 14))
        .simultaneousGesture(TapGesture().onEnded { self.isFocused = true })
        .shadow(color: self.theme.colors.text.opacity(0.08), radius: 5, y: 2)
        .frame(maxWidth: .infinity)
        .padding(.bottom, 4)
        .onAppear {
            self.isFocused = true
        }
        .transition(.opacity)
    }

    private var historyButton: some View {
        Button(action: self.onToggleHistory) {
            Image(systemName: self.showsHistory ? "clock.fill" : "clock")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(
                    self.showsHistory
                        ? self.theme.colors.background
                        : self.theme.colors.textMuted
                )
                .frame(width: 30, height: 30)
                .background(
                    Circle().fill(
                        self.showsHistory
                            ? self.theme.colors.primary
                            : self.theme.colors.background
                    )
                )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(self.historyLabel)
        .accessibilityIdentifier("overlay.composer.history")
        .help(self.historyLabel)
    }

    private var sendButton: some View {
        Button(action: self.handleSubmit) {
            Image(systemName: "arrow.up")
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(
                    self.hasText ? self.theme.colors.background : self.theme.colors.textMuted
                )
                .frame(width: 32, height: 32)
                .background(
                    Circle().fill(
                        self.hasText ? self.theme.colors.secondary : self.theme.colors.border
                    )
                )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L10n.tr("overlay.input.send.accessibility"))
        .accessibilityIdentifier("overlay.composer.send")
        .contentShape(Circle())
        .disabled(!self.hasText)
        .transition(.opacity)
    }

    private var closeButton: some View {
        Button(action: self.onClose) {
            Image(systemName: "xmark")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(self.theme.colors.textMuted)
                .frame(width: 30, height: 30)
                .background(
                    Circle().fill(self.theme.colors.background)
                )
                .overlay(
                    Circle().stroke(
                        self.theme.colors.border.opacity(self.isCloseHovered ? 0.9 : 0.55),
                        lineWidth: 1
                    )
                )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
        .accessibilityIdentifier("overlay.composer.close")
        .contentShape(Circle())
        .onHover { self.isCloseHovered = $0 }
    }

    private var historyLabel: String {
        L10n.tr(
            self.showsHistory
                ? "overlay.input.history.hide.accessibility"
                : "overlay.input.history.show.accessibility"
        )
    }

    private var hasText: Bool {
        self.text.contains(where: { !$0.isWhitespace })
    }

    private func handleSubmit() {
        guard self.hasText else { return }
        let trimmed = self.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard self.onSend(trimmed) else { return }
        self.text = ""
    }

    private var placeholderColor: Color {
        self.theme.isDark ? self.theme.colors.text.opacity(0.72) : self.theme.colors.textMuted.opacity(0.92)
    }

    private var inputTextColor: Color {
        self.theme.isDark ? self.theme.colors.text.opacity(0.98) : self.theme.colors.text
    }
}

// MARK: - Previews

#if !SPM_BUILD
    #Preview("Message Input") {
        MessageInput(text: .constant(""), onSend: { _ in true }, onClose: {})
            .environment(\.theme, allThemes[0])
            .frame(width: 500)
            .padding()
            .background(Color.gray.opacity(0.1))
    }

    #Preview("Message Input - With Text") {
        MessageInput(
            text: .constant("Some draft text"),
            onSend: { _ in true },
            onClose: {},
            feedbackMessage: "Waiting for BoBe to finish thinking.",
            isBusy: true
        )
        .environment(\.theme, allThemes[0])
        .frame(width: 500)
        .padding()
        .background(Color.gray.opacity(0.1))
    }
#endif

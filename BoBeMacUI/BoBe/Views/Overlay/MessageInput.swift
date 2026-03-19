import SwiftUI

struct MessageInput: View {
    @Binding var text: String
    let onSend: (String) -> Bool
    let onClose: () -> Void
    var feedbackMessage: String?
    var isBusy = false

    @FocusState private var isFocused: Bool
    @Environment(\.theme) private var theme
    @State private var isCloseHovered = false

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 0) {
                Rectangle()
                    .fill(self.theme.colors.secondary)
                    .frame(height: 3)

                VStack(alignment: .leading, spacing: 8) {
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
                        .transition(.opacity.combined(with: .move(edge: .bottom)))
                    }

                    HStack(alignment: .bottom, spacing: 8) {
                        TextField(
                            "",
                            text: self.$text,
                            prompt: Text(L10n.tr("overlay.input.placeholder.default"))
                                .foregroundStyle(self.placeholderColor),
                            axis: .vertical
                        )
                        .padding(EdgeInsets(top: 0, leading: 2, bottom: 6, trailing: 0))
                        .textFieldStyle(.plain)
                        .bobeTextStyle(.inputField)
                        .lineSpacing(2)
                        .foregroundStyle(self.inputTextColor)
                        .tint(self.theme.colors.primary)
                        .lineLimit(1 ... 4)
                        .focused(self.$isFocused)
                        .onSubmit { self.handleSubmit() }
                        .onKeyPress(.escape) {
                            self.onClose()
                            return .handled
                        }

                        Button(action: self.handleSubmit) {
                            ZStack {
                                Circle()
                                    .fill(self.hasText ? self.theme.colors.secondary : self.theme.colors.border)
                                    .frame(width: 40, height: 40)

                                Image(systemName: self.isBusy && self.hasText ? "hourglass" : "arrow.up")
                                    .font(.system(size: 12, weight: .bold))
                                    .foregroundStyle(
                                        self.hasText ? self.theme.colors.background : self.theme.colors.textMuted
                                    )
                            }
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(L10n.tr("overlay.input.send.accessibility"))
                        .frame(width: 40, height: 40)
                        .contentShape(Circle())
                        .disabled(!self.hasText)

                        Button(action: self.onClose) {
                            ZStack {
                                Circle()
                                    .fill(
                                        self.isCloseHovered
                                            ? self.theme.colors.background.opacity(self.theme.isDark ? 0.9 : 0.98)
                                            : self.theme.colors.background.opacity(self.theme.isDark ? 0.14 : 0.08)
                                    )

                                Circle()
                                    .stroke(self.theme.colors.border.opacity(self.isCloseHovered ? 0.9 : 0.65), lineWidth: 1)

                                Image(systemName: "xmark")
                                    .font(.system(size: 10, weight: .semibold))
                                    .foregroundStyle(self.theme.colors.textMuted)
                            }
                            .frame(width: 44, height: 44)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
                        .frame(width: 44, height: 44)
                        .contentShape(Circle())
                        .onHover { hovering in
                            self.isCloseHovered = hovering
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 9)
            }
            .background(self.theme.colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(self.theme.colors.border, lineWidth: 1.5)
            )
            .contentShape(RoundedRectangle(cornerRadius: 16))
            .simultaneousGesture(
                TapGesture().onEnded {
                    self.isFocused = true
                }
            )
            .shadow(color: Color.black.opacity(0.08), radius: 4, y: 2)
        }
        .frame(maxWidth: .infinity)
        .padding(.bottom, 4)
        .onAppear { self.isFocused = true }
        .transition(
            OverlayMotionRuntime.reduceMotion
                ? .opacity
                : .asymmetric(
                    insertion: .opacity.combined(with: .scale(scale: 0.98, anchor: .bottomTrailing)),
                    removal: .opacity
                )
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

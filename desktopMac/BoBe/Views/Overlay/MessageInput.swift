import SwiftUI

/// Text input panel for sending messages.
struct MessageInput: View {
    let onSend: (String) -> Void
    let onClose: () -> Void
    var isThinking = false

    @State private var text = ""
    @FocusState private var isFocused: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 0) {
                Rectangle()
                    .fill(self.theme.colors.secondary)
                    .frame(height: 3)

                HStack(alignment: .bottom, spacing: 8) {
                    TextField(
                        "",
                        text: self.$text,
                        prompt: Text(self.isThinking ? "BoBe is thinking…" : "Type a message...")
                            .foregroundStyle(self.placeholderColor),
                        axis: .vertical
                    )
                    .padding(EdgeInsets(top: 0, leading: 2, bottom: 6, trailing: 0))
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
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

                    if self.isThinking, !self.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("waiting...")
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.textMuted)
                            .lineLimit(1)
                    }

                    Button(action: self.handleSubmit) {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundStyle(
                                self.canSend ? self.theme.colors.background : self.theme.colors.textMuted
                            )
                    }
                    .buttonStyle(.plain)
                    .frame(width: 32, height: 32)
                    .background(
                        Circle()
                            .fill(self.canSend ? self.theme.colors.secondary : self.theme.colors.border)
                    )
                    .disabled(!self.canSend)

                    Button(action: self.onClose) {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .buttonStyle(.plain)
                    .frame(width: 24, height: 24)
                    .background(
                        Circle()
                            .fill(Color.clear)
                    )
                    .contentShape(Circle())
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
            .shadow(color: Color.black.opacity(0.08), radius: 4, y: 2)
        }
        .frame(maxWidth: .infinity)
        .padding(.leading, 16)
        .padding(.bottom, 4)
        .onAppear { self.isFocused = true }
        .transition(
            .asymmetric(
                insertion: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.9, anchor: .bottomTrailing)),
                removal: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.95, anchor: .bottomTrailing))
            )
        )
    }

    private var canSend: Bool {
        !self.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !self.isThinking
    }

    private func handleSubmit() {
        guard self.canSend else { return }
        let trimmed = self.text.trimmingCharacters(in: .whitespacesAndNewlines)
        self.text = ""
        self.onSend(trimmed)
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
    MessageInput(onSend: { _ in }, onClose: {})
        .environment(\.theme, allThemes[0])
        .frame(width: 500)
        .padding()
        .background(Color.gray.opacity(0.1))
}

#Preview("Message Input - Thinking") {
    MessageInput(onSend: { _ in }, onClose: {}, isThinking: true)
        .environment(\.theme, allThemes[0])
        .frame(width: 500)
        .padding()
        .background(Color.gray.opacity(0.1))
}
#endif

import SwiftUI

/// Text input panel for sending messages. Based on compact MessageInput exactly.
/// CSS: .message-input-container-compact with .message-input-accent (3px olive bar)
struct MessageInput: View {
    let onSend: (String) -> Void
    let onClose: () -> Void
    var isThinking: Bool = false

    @State private var text = ""
    @FocusState private var isFocused: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            // Compact container with accent bar
            VStack(spacing: 0) {
                // 3px olive accent bar at top
                Rectangle()
                    .fill(theme.colors.secondary)
                    .frame(height: 3)

                // Content area: textarea + buttons (padding: 8px 12px)
                HStack(alignment: .bottom, spacing: 8) {
                    // Text field (13px, flexible height)
                    TextField(
                        "",
                        text: $text,
                        prompt: Text(isThinking ? "BoBe is thinking…" : "Type a message...")
                            .foregroundStyle(placeholderColor),
                        axis: .vertical,
                            
                    )
                    .padding(EdgeInsets(top: 0, leading: 2, bottom: 6, trailing: 0))
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
                    .lineSpacing(2)
                    .foregroundStyle(inputTextColor)
                    .tint(theme.colors.primary)
                    .lineLimit(1...4)
                    .focused($isFocused)
                    .onSubmit { handleSubmit() }
                    .onKeyPress(.escape) {
                        onClose()
                        return .handled
                    }

                    // Thinking hint (shown while drafting during thinking)
                    if isThinking && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("waiting...")
                            .font(.system(size: 11))
                            .foregroundStyle(theme.colors.textMuted)
                            .lineLimit(1)
                    }

                    // Send button (32px circle)
                    Button(action: handleSubmit) {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundStyle(
                                canSend ? theme.colors.background : theme.colors.textMuted
                            )
                    }
                    .buttonStyle(.plain)
                    .frame(width: 32, height: 32)
                    .background(
                        Circle()
                            .fill(canSend ? theme.colors.secondary : theme.colors.border)
                    )
                    .disabled(!canSend)

                    // Close button (inline, right side)
                    Button(action: onClose) {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
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
            .background(theme.colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(theme.colors.border, lineWidth: 1.5)
            )
            .shadow(color: Color.black.opacity(0.08), radius: 4, y: 2)
        }
        .frame(maxWidth: .infinity)
        .padding(.leading, 16)
        .padding(.bottom, 4)
        .onAppear { isFocused = true }
        .transition(
            .asymmetric(
                insertion: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.9, anchor: .bottomTrailing)),
                removal: .move(edge: .bottom).combined(with: .opacity).combined(with: .scale(scale: 0.95, anchor: .bottomTrailing))
            )
        )
    }

    private var canSend: Bool {
        !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !isThinking
    }

    private func handleSubmit() {
        guard canSend else { return }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        text = ""
        onSend(trimmed)
    }

    private var placeholderColor: Color {
        theme.isDark ? theme.colors.text.opacity(0.72) : theme.colors.textMuted.opacity(0.92)
    }

    private var inputTextColor: Color {
        theme.isDark ? theme.colors.text.opacity(0.98) : theme.colors.text
    }
}

// MARK: - Previews

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

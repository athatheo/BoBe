import SwiftUI

// MARK: - Setup Helper Components

struct SetupModelCard: View {
    let size: ModelSize
    let isSelected: Bool
    let onSelect: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(size.displayName).font(.system(size: 14, weight: .semibold)).foregroundStyle(theme.colors.text)
                Text(size.sizeDescription).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
                Text(size.description).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            }
            Spacer()
            Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(isSelected ? theme.colors.primary : theme.colors.border)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.colors.surface)
                .stroke(isSelected ? theme.colors.primary : theme.colors.border, lineWidth: 1)
        )
        .onTapGesture { onSelect() }
    }
}

struct SetupStepDot: View {
    let label: String
    let done: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 4) {
            Circle().fill(done ? theme.colors.primary : theme.colors.border).frame(width: 12, height: 12)
                .overlay {
                    if done {
                        Image(systemName: "checkmark").font(.system(size: 7, weight: .bold)).foregroundStyle(.white)
                    }
                }
            Text(label).font(.system(size: 9)).foregroundStyle(theme.colors.textMuted)
        }
    }
}

struct SetupFeatureCard: View {
    let title: String
    let description: String
    let granted: Bool
    let badge: String
    var action: (() -> Void)?
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).font(.system(size: 13, weight: .semibold))
                Spacer()
                Text(badge)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(granted ? theme.colors.secondary : theme.colors.textMuted)
                    .padding(.horizontal, 8).padding(.vertical, 2)
                    .background(RoundedRectangle(cornerRadius: 8)
                        .fill(granted ? theme.colors.secondary.opacity(0.15) : theme.colors.surface))
            }
            Text(description).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            if let action, !granted {
                Button("Set Up") { action() }
                    .font(.system(size: 11, weight: .medium)).foregroundStyle(theme.colors.primary).buttonStyle(.plain)
            }
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 10).fill(theme.colors.surface))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(theme.colors.border, lineWidth: 1))
    }
}

struct SetupSummaryRow: View {
    let icon: String
    let color: Color
    let text: String
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon).foregroundStyle(color).font(.system(size: 14))
            Text(text).font(.system(size: 13)).foregroundStyle(theme.colors.text)
        }
    }
}

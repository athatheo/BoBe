import SwiftUI

// MARK: - Welcome Bullet Point

struct WelcomeBullet: View {
    let title: String
    let desc: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
            Text(desc)
                .font(.system(size: 12))
                .foregroundStyle(Color(red: 0.4, green: 0.4, blue: 0.4))
                .lineSpacing(2)
        }
    }
}

// MARK: - Step Indicator (downloading progress steps)

struct StepIndicator: View {
    let label: String
    let active: Bool
    let done: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            Text(done ? "✓" : active ? "●" : "○")
                .font(.system(size: 12))
                .foregroundStyle(
                    done ? Color(red: 0.55, green: 0.6, blue: 0.49)
                        : active ? theme.colors.text : theme.colors.textMuted
                )
            Text(label)
                .font(.system(size: 14, weight: active ? .semibold : .regular))
                .foregroundStyle(
                    active ? theme.colors.text
                        : done ? Color(red: 0.55, green: 0.6, blue: 0.49)
                        : theme.colors.textMuted
                )
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Permission Badge

struct PermissionBadge: View {
    let status: String  // "granted", "denied", "restricted", "not-determined"

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(textColor)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(bgColor)
            )
    }

    private var text: String {
        switch status {
        case "granted": "✓ Granted"
        case "denied": "Not Granted"
        case "restricted": "Restricted"
        default: "Not Set"
        }
    }

    private var textColor: Color {
        switch status {
        case "granted": Color(red: 0.18, green: 0.49, blue: 0.2)
        case "denied": Color(red: 0.78, green: 0.16, blue: 0.16)
        case "restricted": Color(red: 0.9, green: 0.32, blue: 0.0)
        default: Color(red: 0.65, green: 0.56, blue: 0.5)
        }
    }

    private var bgColor: Color {
        switch status {
        case "granted": Color(red: 0.91, green: 0.96, blue: 0.91)
        case "denied": Color(red: 1.0, green: 0.92, blue: 0.93)
        case "restricted": Color(red: 1.0, green: 0.95, blue: 0.88)
        default: Color(red: 0.96, green: 0.94, blue: 0.92)
        }
    }
}

// MARK: - Summary Row

struct SummaryRow: View {
    let label: String
    let value: String
    let ok: Bool
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            Text(ok ? "✓" : "⚠")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(ok ? Color(red: 0.18, green: 0.49, blue: 0.2) : Color(red: 0.9, green: 0.32, blue: 0.0))
            Text("\(label):")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(theme.colors.text)
            Text(value)
                .font(.system(size: 13))
                .foregroundStyle(theme.colors.textMuted)
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(ok ? Color(red: 0.94, green: 0.97, blue: 0.93) : Color(red: 1.0, green: 0.97, blue: 0.96))
                .stroke(ok ? Color(red: 0.77, green: 0.87, blue: 0.72) : Color(red: 0.94, green: 0.84, blue: 0.77), lineWidth: 1)
        )
    }
}

// MARK: - Model Radio Card (local models)

struct ModelRadioCard: View {
    let model: ModelOption
    let isSelected: Bool
    let onSelect: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: isSelected ? "largecircle.fill.circle" : "circle")
                .foregroundStyle(isSelected ? theme.colors.primary : theme.colors.border)
                .font(.system(size: 14))
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(model.label).font(.system(size: 13, weight: .semibold))
                    Text(model.size).font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                }
                Text(model.description)
                    .font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            }
            Spacer()
        }
        .padding(10)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(theme.colors.surface)
                .stroke(isSelected ? theme.colors.primary : theme.colors.border, lineWidth: 1)
        )
        .contentShape(Rectangle())
        .onTapGesture { onSelect() }
    }
}

// MARK: - Permission Card (capture setup)

struct PermissionCard<Content: View>: View {
    let title: String
    let badge: String
    @ViewBuilder let content: () -> Content
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(theme.colors.text)
                Spacer()
                PermissionBadge(status: badge)
            }
            content()
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.colors.surface)
                .stroke(theme.colors.border, lineWidth: 1)
        )
    }
}

// MARK: - Info Card (Soul / Goals on complete step)

struct InfoCard: View {
    let title: String
    let description: String
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(theme.colors.text)
            Text(description)
                .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                .lineSpacing(2)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.colors.surface)
                .stroke(theme.colors.border, lineWidth: 1)
        )
    }
}

// MARK: - Collapsible Section

struct SetupCollapsibleSection<Content: View>: View {
    let title: String
    let collapsedTitle: String
    @Binding var isExpanded: Bool
    @ViewBuilder let content: () -> Content
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 0) {
            Button {
                withAnimation(.easeInOut(duration: 0.2)) { isExpanded.toggle() }
            } label: {
                HStack(spacing: 6) {
                    Text(isExpanded ? "▼" : "▶").font(.system(size: 8))
                    Text(isExpanded ? collapsedTitle : title)
                        .font(.system(size: 13))
                }
                .foregroundStyle(theme.colors.primary)
            }
            .buttonStyle(.plain)

            if isExpanded {
                VStack(alignment: .leading, spacing: 10) {
                    content()
                }
                .padding(12)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(theme.colors.surface)
                        .stroke(theme.colors.border, lineWidth: 1)
                )
                .padding(.top, 8)
            }
        }
    }
}

import SwiftUI

// MARK: - Concept Card (welcome screen)

struct ConceptCard: View {
    let icon: String
    let title: String
    let description: String
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: icon)
                .font(.system(size: 20))
                .foregroundStyle(theme.colors.primary)
                .frame(width: 28)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(theme.colors.text)
                Text(description)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.colors.textMuted)
                    .lineSpacing(2)
            }
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

// MARK: - AI Choice Card

struct AIChoiceCard: View {
    let icon: String
    let title: String
    let subtitle: String
    let action: () -> Void
    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            VStack(spacing: 10) {
                Image(systemName: icon)
                    .font(.system(size: 32))
                    .foregroundStyle(theme.colors.primary)
                Text(title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(theme.colors.text)
                Text(subtitle)
                    .font(.system(size: 12))
                    .foregroundStyle(theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 24)
            .padding(.horizontal, 12)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(theme.colors.surface)
                    .stroke(
                        isHovered ? theme.colors.primary.opacity(0.55) : theme.colors.border,
                        lineWidth: 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .scaleEffect(isHovered ? 1.01 : 1.0)
        .animation(.easeOut(duration: 0.12), value: isHovered)
    }
}

// MARK: - Permission Badge

struct PermissionBadge: View {
    let status: String
    @Environment(\.theme) private var theme

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(textColor)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(textColor.opacity(0.1))
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
        case "granted": theme.colors.secondary
        case "denied": theme.colors.primary
        case "restricted": theme.colors.tertiary
        default: theme.colors.textMuted
        }
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
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(theme.colors.text)
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

// MARK: - Previews

#Preview("Concept Card") {
    VStack(spacing: 12) {
        ConceptCard(icon: "brain.head.profile", title: "Observes", description: "Watches your screen to understand what you're working on")
        ConceptCard(icon: "lightbulb.fill", title: "Suggests", description: "Offers relevant help based on context")
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 440)
    .padding()
}

#Preview("AI Choice Cards") {
    HStack(spacing: 12) {
        AIChoiceCard(icon: "desktopcomputer", title: "Run Locally", subtitle: "Private, on-device AI", action: {})
        AIChoiceCard(icon: "cloud.fill", title: "Use Cloud", subtitle: "OpenAI, Azure, etc.", action: {})
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 440)
    .padding()
}

#Preview("Permission Badges") {
    HStack(spacing: 8) {
        PermissionBadge(status: "granted")
        PermissionBadge(status: "denied")
        PermissionBadge(status: "restricted")
        PermissionBadge(status: "not-determined")
    }
    .environment(\.theme, allThemes[0])
    .padding()
}

#Preview("Permission Card") {
    PermissionCard(title: "Screen Recording", badge: "granted") {
        Text("Grants BoBe access to see what's on your screen.")
            .font(.system(size: 12))
            .foregroundStyle(allThemes[0].colors.textMuted)
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 440)
    .padding()
}

#Preview("Tier Card") {
    VStack(spacing: 8) {
        TierCard(
            tier: LocalTier(id: "small", label: "Compact (4B)", description: "Fast, lightweight", diskEstimateBytes: 6_000_000_000),
            isSelected: true,
            onSelect: {}
        )
        TierCard(
            tier: LocalTier(id: "medium", label: "Balanced (8B)", description: "Best balance of speed and quality", diskEstimateBytes: 11_000_000_000),
            isSelected: false,
            onSelect: {}
        )
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 440)
    .padding()
}

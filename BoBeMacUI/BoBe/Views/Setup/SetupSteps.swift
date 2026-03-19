import SwiftUI

struct ConceptCard: View {
    let icon: String
    let title: String
    let description: String
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: self.icon)
                .font(.system(size: 20))
                .foregroundStyle(self.theme.colors.primary)
                .frame(width: 28)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 2) {
                Text(self.title)
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(self.description)
                    .bobeTextStyle(.inputField)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .lineSpacing(2)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(self.theme.colors.surface)
                .stroke(self.theme.colors.border, lineWidth: 1)
        )
    }
}

struct AIChoiceCard: View {
    let icon: String
    let title: String
    let subtitle: String
    let action: () -> Void
    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button(action: self.action) {
            VStack(spacing: 10) {
                Image(systemName: self.icon)
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.primary)
                Text(self.title)
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(self.subtitle)
                    .bobeTextStyle(.body)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 24)
            .padding(.horizontal, 12)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(self.theme.colors.surface)
                    .stroke(
                        self.isHovered ? self.theme.colors.primary.opacity(0.55) : self.theme.colors.border,
                        lineWidth: 1
                    )
            )
        }
        .buttonStyle(.plain)
        .onHover { self.isHovered = $0 }
        .scaleEffect(self.isHovered ? 1.01 : 1.0)
        .animation(OverlayMotionRuntime.reduceMotion ? nil : .easeOut(duration: 0.12), value: self.isHovered)
    }
}

struct PermissionBadge: View {
    let status: ScreenPermissionStatus
    @Environment(\.theme) private var theme

    var body: some View {
        Text(self.text)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(self.textColor)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.textColor.opacity(0.1))
            )
    }

    private var text: String {
        switch self.status {
        case .granted: L10n.tr("setup.permission_status.granted")
        case .denied: L10n.tr("setup.permission_status.denied")
        case .restricted: L10n.tr("setup.permission_status.restricted")
        case .notDetermined: L10n.tr("setup.permission_status.not_set")
        }
    }

    private var textColor: Color {
        switch self.status {
        case .granted: self.theme.colors.secondary
        case .denied: self.theme.colors.primary
        case .restricted: self.theme.colors.tertiary
        case .notDetermined: self.theme.colors.textMuted
        }
    }
}

struct PermissionCard<Content: View>: View {
    let title: String
    let badge: ScreenPermissionStatus
    @ViewBuilder let content: () -> Content
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(self.title)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(self.theme.colors.text)
                Spacer()
                PermissionBadge(status: self.badge)
            }
            self.content()
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(self.theme.colors.surface)
                .stroke(self.theme.colors.border, lineWidth: 1)
        )
    }
}

#if !SPM_BUILD
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
        PermissionBadge(status: .granted)
        PermissionBadge(status: .denied)
        PermissionBadge(status: .restricted)
        PermissionBadge(status: .notDetermined)
    }
    .environment(\.theme, allThemes[0])
    .padding()
}

#Preview("Permission Card") {
    PermissionCard(title: "Screen Recording", badge: .granted) {
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
            tier: LocalTier(
                id: "medium", label: "Balanced (8B)", description: "Best balance of speed and quality", diskEstimateBytes: 11_000_000_000
            ),
            isSelected: false,
            onSelect: {}
        )
    }
    .environment(\.theme, allThemes[0])
    .frame(width: 440)
    .padding()
}
#endif

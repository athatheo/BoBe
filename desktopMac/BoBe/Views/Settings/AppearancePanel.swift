import SwiftUI

/// Appearance settings panel — theme picker with visual preview cards.
/// Matches Electron AppearanceSettings.tsx with mini avatar preview.
struct AppearancePanel: View {
    @State private var themeStore = ThemeStore.shared
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 8) {
                    Image(systemName: "paintpalette.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(theme.colors.primary)
                    Text("Theme")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(theme.colors.text)
                }

                Text("Choose a color theme for BoBe. This affects the avatar and all UI elements.")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.colors.textMuted)

                LazyVGrid(columns: [
                    GridItem(.flexible(), spacing: 16),
                    GridItem(.flexible(), spacing: 16),
                    GridItem(.flexible(), spacing: 16)
                ], spacing: 16) {
                    ForEach(allThemes) { themeConfig in
                        ThemeCard(
                            themeConfig: themeConfig,
                            isSelected: themeConfig.themeId == themeStore.themeId,
                            onSelect: { themeStore.setTheme(themeConfig.themeId) }
                        )
                    }
                }
            }
            .padding(24)
        }
    }
}

struct ThemeCard: View {
    let themeConfig: ThemeConfig
    let isSelected: Bool
    let onSelect: () -> Void
    @State private var isHovered = false

    var body: some View {
        VStack(spacing: 10) {
            // Mini avatar preview
            ZStack {
                // Outer ring
                Circle()
                    .fill(themeConfig.colors.avatarRing)
                    .frame(width: 48, height: 48)
                    .overlay(
                        Circle()
                            .stroke(themeConfig.colors.border, lineWidth: 1.5)
                    )

                // Face gradient
                Circle()
                    .fill(
                        LinearGradient(
                            colors: [themeConfig.colors.avatarFaceLight, themeConfig.colors.avatarFaceDark],
                            startPoint: .topLeading, endPoint: .bottomTrailing
                        )
                    )
                    .frame(width: 32, height: 32)

                // Closed eyes (sleeping)
                HStack(spacing: 6) {
                    ClosedEyeArc(color: themeConfig.colors.avatarRing)
                    ClosedEyeArc(color: themeConfig.colors.avatarRing)
                }
                .offset(y: 1)
            }

            // Color swatches
            HStack(spacing: 4) {
                Circle().fill(themeConfig.colors.primary).frame(width: 14, height: 14)
                Circle().fill(themeConfig.colors.secondary).frame(width: 14, height: 14)
                Circle().fill(themeConfig.colors.tertiary).frame(width: 14, height: 14)
            }

            // Theme info
            HStack(spacing: 4) {
                Text(themeConfig.name)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(themeConfig.colors.text)
                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(themeConfig.colors.primary)
                }
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(themeConfig.colors.surface)
                .shadow(color: .black.opacity(0.05), radius: 4, y: 2)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(isSelected ? themeConfig.colors.primary : themeConfig.colors.border, lineWidth: isSelected ? 2 : 1)
        )
        .scaleEffect(isHovered ? 1.02 : 1.0)
        .animation(.easeOut(duration: 0.15), value: isHovered)
        .onHover { isHovered = $0 }
        .onTapGesture(perform: onSelect)
    }
}

/// Mini closed eye arc for theme card preview
private struct ClosedEyeArc: View {
    let color: Color

    var body: some View {
        Path { path in
            path.move(to: CGPoint(x: 0, y: 4))
            path.addQuadCurve(to: CGPoint(x: 6, y: 4), control: CGPoint(x: 3, y: 0))
        }
        .stroke(color, style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
        .frame(width: 6, height: 5)
    }
}

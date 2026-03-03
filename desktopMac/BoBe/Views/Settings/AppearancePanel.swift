import SwiftUI

/// Appearance settings panel — theme picker with visual preview cards.
struct AppearancePanel: View {
    @State private var themeStore = ThemeStore.shared
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 8) {
                    Image(systemName: "paintpalette.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(self.theme.colors.primary)
                    Text("Theme")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(self.theme.colors.text)
                }

                Text("Choose a color theme for BoBe. This affects the avatar and all UI elements.")
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.textMuted)

                LazyVGrid(
                    columns: [
                        GridItem(.flexible(), spacing: 16),
                        GridItem(.flexible(), spacing: 16),
                        GridItem(.flexible(), spacing: 16),
                    ], spacing: 16
                ) {
                    ForEach(allThemes) { themeConfig in
                        ThemeCard(
                            themeConfig: themeConfig,
                            isSelected: themeConfig.themeId == self.themeStore.themeId,
                            onSelect: { self.themeStore.setTheme(themeConfig.themeId) }
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
        Button(action: self.onSelect) {
            VStack(alignment: .leading, spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 10)
                        .fill(self.themeConfig.colors.background)
                        .overlay(
                            RoundedRectangle(cornerRadius: 10)
                                .stroke(self.themeConfig.colors.border, lineWidth: 1)
                        )

                    VStack(spacing: 8) {
                        ZStack {
                            Circle()
                                .fill(self.themeConfig.colors.avatarRing)
                                .frame(width: 48, height: 48)
                                .overlay(
                                    Circle()
                                        .stroke(self.themeConfig.colors.border, lineWidth: 1.5)
                                )

                            Circle()
                                .fill(
                                    LinearGradient(
                                        colors: [self.themeConfig.colors.avatarFaceLight, self.themeConfig.colors.avatarFaceDark],
                                        startPoint: .topLeading, endPoint: .bottomTrailing
                                    )
                                )
                                .frame(width: 32, height: 32)

                            HStack(spacing: 6) {
                                ClosedEyeArc(color: self.themeConfig.colors.avatarRing)
                                ClosedEyeArc(color: self.themeConfig.colors.avatarRing)
                            }
                            .offset(y: 1)
                        }

                        HStack(spacing: 4) {
                            Circle().fill(self.themeConfig.colors.primary).frame(width: 14, height: 14)
                            Circle().fill(self.themeConfig.colors.secondary).frame(width: 14, height: 14)
                            Circle().fill(self.themeConfig.colors.tertiary).frame(width: 14, height: 14)
                        }
                    }
                }
                .frame(height: 92)

                HStack(spacing: 4) {
                    Text(self.themeConfig.name)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(self.themeConfig.colors.text)
                    if self.isSelected {
                        Image(systemName: "checkmark")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(self.themeConfig.colors.primary)
                    }
                }

                Text(self.themeConfig.description)
                    .font(.system(size: 10))
                    .foregroundStyle(self.themeConfig.colors.textMuted)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, minHeight: 152, alignment: .topLeading)
            .padding(14)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(self.themeConfig.colors.surface)
                    .shadow(color: .black.opacity(0.05), radius: 4, y: 2)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(self.isSelected ? self.themeConfig.colors.primary : self.themeConfig.colors.border, lineWidth: self.isSelected ? 2 : 1)
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(self.themeConfig.name) theme")
        .accessibilityAddTraits(self.isSelected ? .isSelected : [])
        .scaleEffect(self.isHovered ? 1.02 : 1.0)
        .animation(.easeOut(duration: 0.15), value: self.isHovered)
        .onHover { self.isHovered = $0 }
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
        .stroke(self.color, style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
        .frame(width: 6, height: 5)
    }
}

// MARK: - Previews

#Preview("Appearance Panel") {
    AppearancePanel()
        .environment(\.theme, allThemes[0])
        .frame(width: 600, height: 500)
}

#Preview("Theme Card") {
    HStack(spacing: 16) {
        ThemeCard(themeConfig: allThemes[0], isSelected: true, onSelect: {})
        ThemeCard(themeConfig: allThemes[1], isSelected: false, onSelect: {})
    }
    .padding()
    .frame(width: 500)
}

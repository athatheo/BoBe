import AppKit
import SwiftUI

enum ThemeId: String, CaseIterable, Sendable {
    case system
    case bauhaus
    case bauhausPastel = "bauhaus-pastel"
    case cute
    case cutePastel = "cute-pastel"
    case bauhausDark = "bauhaus-dark"
    case cuteDark = "cute-dark"
}

struct ThemeColors: Sendable {
    let primary: Color
    let secondary: Color
    let tertiary: Color
    let background: Color
    let surface: Color
    let border: Color
    let text: Color
    let textMuted: Color
    let avatarFaceLight: Color
    let avatarFaceDark: Color
    let avatarRing: Color
    let avatarIris: Color
    let avatarEyeOutline: Color
    let avatarMouth: Color
    let success: Color
    let warning: Color
    let error: Color
    let destructive: Color
}

struct ThemeConfig: Identifiable, Sendable {
    var id: ThemeId {
        self.themeId
    }

    let themeId: ThemeId
    let nameKey: String
    let descriptionKey: String
    let isDark: Bool
    let colors: ThemeColors

    var name: String {
        L10n.tr(self.nameKey)
    }

    var description: String {
        L10n.tr(self.descriptionKey)
    }
}

let allThemes: [ThemeConfig] = [
    ThemeConfig(
        themeId: .bauhaus,
        nameKey: "theme.bauhaus.name",
        descriptionKey: "theme.bauhaus.description",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "94472E"),
            secondary: Color(hex: "526A48"),
            tertiary: Color(hex: "70584C"),
            background: Color(hex: "FAF7F2"),
            surface: Color(hex: "FFFDFC"),
            border: Color(hex: "D8C9B4"),
            text: Color(hex: "3A3A3A"),
            textMuted: Color(hex: "62605D"),
            avatarFaceLight: Color(hex: "E8DCC4"),
            avatarFaceDark: Color(hex: "B8A99A"),
            avatarRing: Color(hex: "FAF7F2"),
            avatarIris: Color(hex: "A69080"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "94472E"),
            success: Color(hex: "2F6F48"),
            warning: Color(hex: "805A00"),
            error: Color(hex: "B3261E"),
            destructive: Color(hex: "B3261E")
        )
    ),
    ThemeConfig(
        themeId: .bauhausPastel,
        nameKey: "theme.bauhaus_pastel.name",
        descriptionKey: "theme.bauhaus_pastel.description",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "865046"),
            secondary: Color(hex: "53694D"),
            tertiary: Color(hex: "6B5C52"),
            background: Color(hex: "FDFBF8"),
            surface: Color(hex: "FFFFFF"),
            border: Color(hex: "DED3C4"),
            text: Color(hex: "4A4A4A"),
            textMuted: Color(hex: "66625F"),
            avatarFaceLight: Color(hex: "EDE5D8"),
            avatarFaceDark: Color(hex: "C9BAA9"),
            avatarRing: Color(hex: "FDFBF8"),
            avatarIris: Color(hex: "B8A99A"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "865046"),
            success: Color(hex: "2F6F48"),
            warning: Color(hex: "805A00"),
            error: Color(hex: "B3261E"),
            destructive: Color(hex: "B3261E")
        )
    ),
    ThemeConfig(
        themeId: .cute,
        nameKey: "theme.cute.name",
        descriptionKey: "theme.cute.description",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "A33E5A"),
            secondary: Color(hex: "3D715F"),
            tertiary: Color(hex: "66518A"),
            background: Color(hex: "FFF8FA"),
            surface: Color(hex: "FFFFFF"),
            border: Color(hex: "E9CFD7"),
            text: Color(hex: "3D3D3D"),
            textMuted: Color(hex: "646062"),
            avatarFaceLight: Color(hex: "FFD4DC"),
            avatarFaceDark: Color(hex: "FFBAC8"),
            avatarRing: Color(hex: "FFF8FA"),
            avatarIris: Color(hex: "E8879C"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "A33E5A"),
            success: Color(hex: "2F6F48"),
            warning: Color(hex: "805A00"),
            error: Color(hex: "B3261E"),
            destructive: Color(hex: "B3261E")
        )
    ),
    ThemeConfig(
        themeId: .cutePastel,
        nameKey: "theme.cute_pastel.name",
        descriptionKey: "theme.cute_pastel.description",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "963D56"),
            secondary: Color(hex: "426B5B"),
            tertiary: Color(hex: "62527B"),
            background: Color(hex: "FFFCFD"),
            surface: Color(hex: "FFFFFF"),
            border: Color(hex: "EBD8DE"),
            text: Color(hex: "4D4D4D"),
            textMuted: Color(hex: "686466"),
            avatarFaceLight: Color(hex: "D8C4E8"),
            avatarFaceDark: Color(hex: "C4B0D8"),
            avatarRing: Color(hex: "FFFCFD"),
            avatarIris: Color(hex: "B8A4D4"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "765988"),
            success: Color(hex: "2F6F48"),
            warning: Color(hex: "805A00"),
            error: Color(hex: "B3261E"),
            destructive: Color(hex: "B3261E")
        )
    ),
    ThemeConfig(
        themeId: .bauhausDark,
        nameKey: "theme.bauhaus_dark.name",
        descriptionKey: "theme.bauhaus_dark.description",
        isDark: true,
        colors: ThemeColors(
            primary: Color(hex: "D4926F"),
            secondary: Color(hex: "9AAD8E"),
            tertiary: Color(hex: "B8A090"),
            background: Color(hex: "1E1E1E"),
            surface: Color(hex: "2A2A2A"),
            border: Color(hex: "3D3D3D"),
            text: Color(hex: "E8E4DF"),
            textMuted: Color(hex: "B3AEA9"),
            avatarFaceLight: Color(hex: "A89070"),
            avatarFaceDark: Color(hex: "8A7560"),
            avatarRing: Color(hex: "2A2A2A"),
            avatarIris: Color(hex: "3A3A3A"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "3A3A3A"),
            success: Color(hex: "82C995"),
            warning: Color(hex: "E4B95A"),
            error: Color(hex: "FF8A80"),
            destructive: Color(hex: "FF8A80")
        )
    ),
    ThemeConfig(
        themeId: .cuteDark,
        nameKey: "theme.cute_dark.name",
        descriptionKey: "theme.cute_dark.description",
        isDark: true,
        colors: ThemeColors(
            primary: Color(hex: "F2A0B0"),
            secondary: Color(hex: "8DCAB5"),
            tertiary: Color(hex: "C4B4E0"),
            background: Color(hex: "1A1A1E"),
            surface: Color(hex: "252528"),
            border: Color(hex: "3A3A40"),
            text: Color(hex: "F5F0F2"),
            textMuted: Color(hex: "B2ADB0"),
            avatarFaceLight: Color(hex: "C89098"),
            avatarFaceDark: Color(hex: "A87080"),
            avatarRing: Color(hex: "252528"),
            avatarIris: Color(hex: "3A3A3A"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "8A4050"),
            success: Color(hex: "82C995"),
            warning: Color(hex: "E4B95A"),
            error: Color(hex: "FF8A80"),
            destructive: Color(hex: "FF8A80")
        )
    ),
]

@MainActor
func themeById(_ id: ThemeId) -> ThemeConfig {
    if id == .system {
        return systemTheme()
    }
    return allThemes.first { $0.themeId == id } ?? allThemes[0]
}

@MainActor
func systemTheme() -> ThemeConfig {
    let isDark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    let base = themeById(isDark ? .bauhausDark : .bauhaus)
    return ThemeConfig(
        themeId: .system,
        nameKey: "theme.system.name",
        descriptionKey: "theme.system.description",
        isDark: isDark,
        colors: base.colors
    )
}

extension EnvironmentValues {
    @Entry var theme: ThemeConfig = allThemes[0]
}

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let r: Double
        let g: Double
        let b: Double
        switch hex.count {
        case 6:
            r = Double((int >> 16) & 0xFF) / 255.0
            g = Double((int >> 8) & 0xFF) / 255.0
            b = Double(int & 0xFF) / 255.0
        default:
            r = 1
            g = 1
            b = 1
        }
        self.init(red: r, green: g, blue: b)
    }
}

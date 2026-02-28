import SwiftUI

// MARK: - Theme Configuration

enum ThemeId: String, CaseIterable, Sendable {
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
}

struct ThemeConfig: Identifiable, Sendable {
    var id: ThemeId { themeId }
    let themeId: ThemeId
    let name: String
    let description: String
    let isDark: Bool
    let colors: ThemeColors
}

// MARK: - All Themes

let allThemes: [ThemeConfig] = [
    ThemeConfig(
        themeId: .bauhaus,
        name: "Terracotta Dreams",
        description: "Warm earthy tones",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "C67B5C"),
            secondary: Color(hex: "8B9A7D"),
            tertiary: Color(hex: "A69080"),
            background: Color(hex: "FAF7F2"),
            surface: Color(hex: "FAF7F2"),
            border: Color(hex: "E8DCC4"),
            text: Color(hex: "3A3A3A"),
            textMuted: Color(hex: "6B6B6B"),
            avatarFaceLight: Color(hex: "E8DCC4"),
            avatarFaceDark: Color(hex: "B8A99A"),
            avatarRing: Color(hex: "FAF7F2"),
            avatarIris: Color(hex: "A69080"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "C67B5C")
        )
    ),
    ThemeConfig(
        themeId: .bauhausPastel,
        name: "Soft Clay",
        description: "Gentle muted warmth",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "D4A59A"),
            secondary: Color(hex: "A8B89F"),
            tertiary: Color(hex: "C4B5A9"),
            background: Color(hex: "FDFBF8"),
            surface: Color(hex: "FDFBF8"),
            border: Color(hex: "EDE5D8"),
            text: Color(hex: "4A4A4A"),
            textMuted: Color(hex: "7A7A7A"),
            avatarFaceLight: Color(hex: "EDE5D8"),
            avatarFaceDark: Color(hex: "C9BAA9"),
            avatarRing: Color(hex: "FDFBF8"),
            avatarIris: Color(hex: "B8A99A"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "D4A59A")
        )
    ),
    ThemeConfig(
        themeId: .cute,
        name: "Bubblegum",
        description: "Playful pink vibes",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "E8879C"),
            secondary: Color(hex: "7DBDA8"),
            tertiary: Color(hex: "B8A4D4"),
            background: Color(hex: "FFF8FA"),
            surface: Color(hex: "FFF8FA"),
            border: Color(hex: "F5E0E5"),
            text: Color(hex: "3D3D3D"),
            textMuted: Color(hex: "6D6D6D"),
            avatarFaceLight: Color(hex: "FFD4DC"),
            avatarFaceDark: Color(hex: "FFBAC8"),
            avatarRing: Color(hex: "FFF8FA"),
            avatarIris: Color(hex: "E8879C"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "E8879C")
        )
    ),
    ThemeConfig(
        themeId: .cutePastel,
        name: "Cotton Candy",
        description: "Dreamy soft pastels",
        isDark: false,
        colors: ThemeColors(
            primary: Color(hex: "F2A6B4"),
            secondary: Color(hex: "A8D5C2"),
            tertiary: Color(hex: "D4C4E8"),
            background: Color(hex: "FFFCFD"),
            surface: Color(hex: "FFFCFD"),
            border: Color(hex: "F8E8EC"),
            text: Color(hex: "4D4D4D"),
            textMuted: Color(hex: "7D7D7D"),
            avatarFaceLight: Color(hex: "D8C4E8"),
            avatarFaceDark: Color(hex: "C4B0D8"),
            avatarRing: Color(hex: "FFFCFD"),
            avatarIris: Color(hex: "B8A4D4"),
            avatarEyeOutline: .white,
            avatarMouth: Color(hex: "9A7AAA")
        )
    ),
    ThemeConfig(
        themeId: .bauhausDark,
        name: "Midnight Clay",
        description: "Warm tones in the dark",
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
            avatarMouth: Color(hex: "3A3A3A")
        )
    ),
    ThemeConfig(
        themeId: .cuteDark,
        name: "Twilight Rose",
        description: "Soft pink in the dark",
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
            avatarMouth: Color(hex: "8A4050")
        )
    ),
]

func themeById(_ id: ThemeId) -> ThemeConfig {
    allThemes.first { $0.themeId == id } ?? allThemes[0]
}

// MARK: - SwiftUI Environment

private struct ThemeEnvironmentKey: EnvironmentKey {
    static let defaultValue: ThemeConfig = allThemes[0]
}

extension EnvironmentValues {
    var theme: ThemeConfig {
        get { self[ThemeEnvironmentKey.self] }
        set { self[ThemeEnvironmentKey.self] = newValue }
    }
}

// MARK: - Color Hex Extension

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let r, g, b: Double
        switch hex.count {
        case 6:
            r = Double((int >> 16) & 0xFF) / 255.0
            g = Double((int >> 8) & 0xFF) / 255.0
            b = Double(int & 0xFF) / 255.0
        default:
            r = 1; g = 1; b = 1
        }
        self.init(red: r, green: g, blue: b)
    }
}

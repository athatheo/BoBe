import Observation
import SwiftUI

/// Manages the active theme with UserDefaults persistence
@Observable
final class ThemeStore: @unchecked Sendable {
    static let shared = ThemeStore()

    private(set) var currentTheme: ThemeConfig
    private let userDefaultsKey = "bobe_theme_id"

    var themeId: ThemeId {
        get { currentTheme.themeId }
        set { setTheme(newValue) }
    }

    private init() {
        let savedId = UserDefaults.standard.string(forKey: "bobe_theme_id")
            .flatMap { ThemeId(rawValue: $0) } ?? .bauhaus
        self.currentTheme = themeById(savedId)
    }

    func setTheme(_ id: ThemeId) {
        currentTheme = themeById(id)
        UserDefaults.standard.set(id.rawValue, forKey: userDefaultsKey)
    }
}

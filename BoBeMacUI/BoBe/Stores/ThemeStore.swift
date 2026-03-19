import Observation
import SwiftUI

@MainActor
@Observable
final class ThemeStore {
    static let shared = ThemeStore()

    private(set) var currentTheme: ThemeConfig
    private static let themeDefaultsKey = "bobe_theme_id"

    var themeId: ThemeId {
        get { self.currentTheme.themeId }
        set { self.setTheme(newValue) }
    }

    private init() {
        let savedId =
            UserDefaults.standard.string(forKey: Self.themeDefaultsKey)
                .flatMap { ThemeId(rawValue: $0) } ?? .bauhaus
        self.currentTheme = themeById(savedId)
    }

    func setTheme(_ id: ThemeId) {
        self.currentTheme = themeById(id)
        UserDefaults.standard.set(id.rawValue, forKey: Self.themeDefaultsKey)
    }
}

import Observation
import SwiftUI

@MainActor
@Observable
final class ThemeStore {
    static let shared = ThemeStore()

    private(set) var currentTheme: ThemeConfig
    private let userDefaultsKey = "bobe_theme_id"

    var themeId: ThemeId {
        get { self.currentTheme.themeId }
        set { self.setTheme(newValue) }
    }

    private init() {
        let savedId =
            UserDefaults.standard.string(forKey: "bobe_theme_id")
                .flatMap { ThemeId(rawValue: $0) } ?? .bauhaus
        self.currentTheme = themeById(savedId)
    }

    func setTheme(_ id: ThemeId) {
        self.currentTheme = themeById(id)
        UserDefaults.standard.set(id.rawValue, forKey: self.userDefaultsKey)
    }
}

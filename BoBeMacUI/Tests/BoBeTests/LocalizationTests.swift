@testable import BoBe
import Testing

@Suite("Localization", .serialized)
struct LocalizationTests {
    @Test
    @MainActor
    func composerHintsExistInEverySupportedLocale() {
        let keys = [
            "overlay.input.placeholder.hint.notice",
            "overlay.input.placeholder.hint.summarize",
            "overlay.input.placeholder.hint.goal",
            "overlay.input.placeholder.hint.feelings",
        ]

        defer { L10n.setLocaleOverride(nil) }
        for locale in BobeStore.supportedLocales {
            L10n.setLocaleOverride(locale)
            for key in keys {
                #expect(L10n.tr(key) != key, "Missing \(key) in \(locale)")
            }
        }
    }

}

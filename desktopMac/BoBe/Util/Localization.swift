import Foundation

enum L10n {
    nonisolated(unsafe) private static var overrideBundle: Bundle?

    static func setLocaleOverride(_ localeId: String?) {
        guard let localeId, !localeId.isEmpty else {
            overrideBundle = nil
            return
        }
        // Try exact match first (e.g. "de-DE"), then language-only (e.g. "de")
        let candidates = [localeId, String(localeId.prefix(2))]
        for candidate in candidates {
            if let url = Bundle.appResources.url(forResource: candidate, withExtension: "lproj"),
               let bundle = Bundle(url: url) {
                overrideBundle = bundle
                return
            }
        }
        overrideBundle = nil
    }

    static func tr(_ key: String, _ args: CVarArg...) -> String {
        let bundle = overrideBundle ?? Bundle.appResources
        let format = NSLocalizedString(
            key,
            tableName: "UI",
            bundle: bundle,
            value: key,
            comment: ""
        )
        guard !args.isEmpty else { return format }
        return String(format: format, locale: .current, arguments: args)
    }
}

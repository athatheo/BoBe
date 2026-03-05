import Foundation

enum L10n {
    static func tr(_ key: String, _ args: CVarArg...) -> String {
        let format = NSLocalizedString(
            key,
            tableName: "UI",
            bundle: .main,
            value: key,
            comment: ""
        )
        guard !args.isEmpty else { return format }
        return String(format: format, locale: .current, arguments: args)
    }
}

import Foundation

/// Mirror of daemon-side validation rules so the UI can surface errors
/// without round-tripping. Keep in lockstep with:
///
/// - `BoBeService/src/services/souls/souls_service.rs` (`MIN_CONTENT_LEN`)
/// - `BoBeService/src/services/user_profile/user_profile_service.rs` (`MIN_CONTENT_LEN`)
/// - `BoBeMacUI/BoBe/Features/Settings/GoalsEditor.swift` (`priorityRange`, also used by daemon range checks if added later)
///
/// An `/api/schema` endpoint would dedupe further; for now the constraints
/// are small + stable, so duplication is the cheapest correct answer.
enum Validations {
    static let soulContentMinLength = 10
    static let userProfileContentMinLength = 10

    static func validateSoulContent(_ content: String) -> String? {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.count < self.soulContentMinLength {
            return String(
                format: L10n.tr("settings.shared.validation.content_min_length_format"),
                self.soulContentMinLength
            )
        }
        return nil
    }

    static func validateUserProfileContent(_ content: String) -> String? {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.count < self.userProfileContentMinLength {
            return String(
                format: L10n.tr("settings.shared.validation.content_min_length_format"),
                self.userProfileContentMinLength
            )
        }
        return nil
    }
}

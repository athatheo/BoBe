import SwiftUI

struct UserProfilesEditor: View {
    var body: some View {
        EntityListEditor<UserProfile, UserProfileCreateRequest, UserProfileUpdateRequest>(
            strings: EntityEditorStrings(
                paneTitle: L10n.tr("settings.user_profiles.title"),
                newPlaceholder: L10n.tr("settings.user_profiles.new.placeholder"),
                loading: L10n.tr("settings.user_profiles.loading"),
                emptyTitle: L10n.tr("settings.user_profiles.empty.title"),
                emptyDescription: L10n.tr("settings.user_profiles.empty.description"),
                emptySelectPrompt: L10n.tr("settings.user_profiles.empty.select"),
                deleteAccessibility: L10n.tr("settings.user_profiles.delete.accessibility"),
                toggleAccessibility: L10n.tr("settings.user_profiles.toggle.enable_accessibility"),
                defaultContent: { name in "# \(name)\n\nDescribe this profile here.\n" }
            ),
            emptyIcon: "person.crop.circle",
            actions: EntityActions(
                list: { try await DaemonClient.shared.listUserProfiles().profiles },
                create: { try await DaemonClient.shared.createUserProfile($0) },
                update: { id, request in try await DaemonClient.shared.updateUserProfile(id, request) },
                delete: { try await DaemonClient.shared.deleteUserProfile($0) },
                enable: { _ = try await DaemonClient.shared.enableUserProfile($0) },
                disable: { _ = try await DaemonClient.shared.disableUserProfile($0) },
                buildCreateRequest: { name in
                    UserProfileCreateRequest(name: name, content: "# \(name)\n\nDescribe this profile here.\n")
                },
                buildUpdateRequest: { content in UserProfileUpdateRequest(content: content) },
                validate: { Validations.validateUserProfileContent($0) }
            )
        )
    }
}

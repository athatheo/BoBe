import SwiftUI

struct SoulsEditor: View {
    var body: some View {
        EntityListEditor<Soul, SoulCreateRequest, SoulUpdateRequest>(
            strings: EntityEditorStrings(
                paneTitle: L10n.tr("settings.souls.title"),
                newPlaceholder: L10n.tr("settings.souls.new.placeholder"),
                loading: L10n.tr("settings.souls.loading"),
                emptyTitle: L10n.tr("settings.souls.empty.title"),
                emptyDescription: L10n.tr("settings.souls.empty.description"),
                emptySelectPrompt: L10n.tr("settings.souls.empty.select"),
                deleteAccessibility: L10n.tr("settings.souls.delete.accessibility"),
                toggleAccessibility: L10n.tr("settings.souls.toggle.enable_accessibility"),
                defaultContent: { name in "# \(name)\n\nDescribe this soul here.\n" }
            ),
            emptyIcon: "sparkles",
            actions: EntityActions(
                list: { try await DaemonClient.shared.listSouls().souls },
                create: { try await DaemonClient.shared.createSoul($0) },
                update: { id, request in try await DaemonClient.shared.updateSoul(id, request) },
                delete: { try await DaemonClient.shared.deleteSoul($0) },
                enable: { _ = try await DaemonClient.shared.enableSoul($0) },
                disable: { _ = try await DaemonClient.shared.disableSoul($0) },
                buildCreateRequest: { name in
                    SoulCreateRequest(name: name, content: "# \(name)\n\nDescribe this soul here.\n")
                },
                buildUpdateRequest: { content in SoulUpdateRequest(content: content) },
                validate: { Validations.validateSoulContent($0) }
            )
        )
    }
}

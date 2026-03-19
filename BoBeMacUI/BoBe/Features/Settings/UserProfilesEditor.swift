import SwiftUI

struct UserProfilesEditor: View {
    @State private var profiles: [UserProfile] = []
    @State private var editorState = SettingsEditorState<String>()
    @State private var editorContent = ""
    @State private var newName = ""
    @Environment(\.theme) private var theme

    private var selectedProfile: UserProfile? {
        self.profiles.first { $0.id == self.editorState.selectedId }
    }

    var body: some View {
        SettingsEditorScaffold(hasSelection: self.selectedProfile != nil) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: L10n.tr("settings.user_profiles.title")) { self.editorState.isCreating.toggle() }
                    .padding(.bottom, 12)

                if self.editorState.isCreating {
                    HStack(spacing: 6) {
                        BobeTextField(placeholder: L10n.tr("settings.user_profiles.new.placeholder"), text: self.$newName) {
                            if !self.newName.isEmpty { self.createProfile() }
                        }
                        Button(L10n.tr("settings.editor.action.create")) { self.createProfile() }
                            .bobeButton(.primary, size: .small)
                            .disabled(self.newName.isEmpty)
                        Button {
                            self.editorState.setCreating(false)
                            self.newName = ""
                        } label: {
                            Text(L10n.tr("settings.editor.action.cancel"))
                        }
                        .bobeButton(.secondary, size: .small)
                    }
                }

                if self.editorState.isCreating, let errorMessage = self.editorState.errorMessage {
                    SettingsEditorErrorText(message: errorMessage)
                }

                if self.editorState.isLoading, self.profiles.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.user_profiles.loading"))
                            .bobeTextStyle(.body)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.profiles.isEmpty, !self.editorState.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "person.crop.circle")
                            .font(.system(size: 28))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text(L10n.tr("settings.user_profiles.empty.title"))
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text(L10n.tr("settings.user_profiles.empty.description"))
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.textMuted.opacity(0.7))
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(self.profiles) { profile in
                                BobeSelectableRow(
                                    isSelected: self.editorState.selectedId == profile.id,
                                    content: {
                                        VStack(alignment: .leading) {
                                            HStack(spacing: 4) {
                                                Text(profile.name)
                                                    .bobeTextStyle(.rowTitle)
                                                if profile.isDefault {
                                                    Text(L10n.tr("settings.editor.badge.default"))
                                                        .bobeTextStyle(.badge)
                                                        .padding(.horizontal, 4)
                                                        .padding(.vertical, 1)
                                                        .background(Capsule().fill(self.theme.colors.primary.opacity(0.2)))
                                                        .foregroundStyle(self.theme.colors.primary)
                                                }
                                            }
                                            Text(String(profile.content.prefix(60)).replacingOccurrences(of: "\n", with: " "))
                                                .bobeTextStyle(.rowMeta)
                                                .foregroundStyle(self.theme.colors.textMuted)
                                                .lineLimit(1)
                                        }
                                        Spacer()
                                        BobeToggle(
                                            isOn: Binding(
                                                get: { profile.enabled },
                                                set: { _ in self.toggleProfile(profile) }
                                            ),
                                            accessibilityLabel: L10n.tr("settings.user_profiles.toggle.enable_accessibility")
                                        )
                                    }
                                )
                                .overlay {
                                    Button { self.editorState.select(profile.id) } label: { Color.clear }
                                        .buttonStyle(.plain)
                                }
                            }
                        }
                    }
                    .background(self.theme.colors.background)
                }
            }
        } detailPane: {
            if let profile = self.selectedProfile {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(profile.name)
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.text)
                        if self.editorState.isDirty {
                            Text(L10n.tr("settings.editor.badge.unsaved"))
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.tertiary.opacity(0.15)))
                        }
                        if profile.isDefault {
                            Text(L10n.tr("settings.editor.badge.default"))
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.primary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.primary.opacity(0.15)))
                        }
                        Spacer()

                        if !profile.isDefault {
                            if self.editorState.showDeleteConfirmation {
                                HStack(spacing: 6) {
                                    Text(L10n.tr("settings.editor.delete.confirm"))
                                        .font(.system(size: 12))
                                        .foregroundStyle(self.theme.colors.primary)
                                    Button(L10n.tr("settings.editor.delete.yes")) {
                                        self.deleteProfile(profile)
                                        self.editorState.dismissDeleteConfirmation()
                                    }
                                    .bobeButton(.destructive, size: .small)
                                    Button(L10n.tr("settings.editor.delete.no")) { self.editorState.dismissDeleteConfirmation() }
                                        .bobeButton(.secondary, size: .small)
                                }
                            } else {
                                Button {
                                    self.editorState.requestDeleteConfirmation()
                                } label: {
                                    Image(systemName: "trash")
                                }
                                .accessibilityLabel(L10n.tr("settings.user_profiles.delete.accessibility"))
                                .bobeButton(.destructive, size: .small)
                            }
                        }

                        SettingsEditorSaveActions(
                            isDirty: self.editorState.isDirty,
                            isSaving: self.editorState.isSaving,
                            onDiscard: self.discardChanges,
                            onSave: self.saveProfile
                        )
                    }

                    CodeEditor(text: self.$editorContent, theme: self.theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: self.editorContent) { _, _ in
                            self.editorState.setDirty(self.editorContent != self.selectedProfile?.content)
                        }

                    if let errorMessage = self.editorState.errorMessage {
                        SettingsEditorErrorText(message: errorMessage)
                    }
                }
            } else {
                EmptyView()
            }
        } emptyPane: {
            VStack(spacing: 8) {
                Image(systemName: "person.crop.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(self.theme.colors.textMuted)
                Text(L10n.tr("settings.user_profiles.empty.select"))
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
        .onChange(of: self.editorState.selectedId) { _, newId in
            if let profile = self.profiles.first(where: { $0.id == newId }) {
                self.editorContent = profile.content
                self.editorState.setDirty(false)
                self.editorState.dismissDeleteConfirmation()
            }
        }
        .task { await self.loadProfiles() }
    }

    private func loadProfiles() async {
        self.editorState.setLoading(true)
        defer { self.editorState.setLoading(false) }
        do {
            let resp = try await DaemonClient.shared.listUserProfiles()
            self.profiles = resp.profiles
            if self.editorState.selectedId == nil {
                self.editorState.select(self.profiles.first?.id)
            }
        } catch {
            self.editorState.setError(error)
        }
    }

    private func createProfile() {
        Task {
            do {
                let profile = try await DaemonClient.shared.createUserProfile(
                    UserProfileCreateRequest(name: self.newName.lowercased(), content: "# \(self.newName)\n\nDescribe this profile here.\n")
                )
                self.profiles.append(profile)
                self.editorState.select(profile.id)
                self.newName = ""
                self.editorState.setCreating(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func saveProfile() {
        guard let id = self.editorState.selectedId else { return }
        self.editorState.setSaving(true)
        Task {
            defer { self.editorState.setSaving(false) }
            do {
                let updated = try await DaemonClient.shared.updateUserProfile(id, UserProfileUpdateRequest(content: self.editorContent))
                if let idx = self.profiles.firstIndex(where: { $0.id == id }) {
                    self.profiles[idx] = updated
                }
                self.editorState.setDirty(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func deleteProfile(_ profile: UserProfile) {
        Task {
            do {
                try await DaemonClient.shared.deleteUserProfile(profile.id)
                self.profiles.removeAll { $0.id == profile.id }
                if self.editorState.selectedId == profile.id {
                    self.editorState.select(self.profiles.first?.id)
                }
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func toggleProfile(_ profile: UserProfile) {
        Task {
            do {
                if profile.enabled {
                    _ = try await DaemonClient.shared.disableUserProfile(profile.id)
                } else {
                    _ = try await DaemonClient.shared.enableUserProfile(profile.id)
                }
                await self.loadProfiles()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func discardChanges() {
        if let profile = self.selectedProfile {
            self.editorContent = profile.content
            self.editorState.setDirty(false)
        }
    }
}

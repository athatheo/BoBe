import SwiftUI

/// Split-pane editor for User Profiles.
/// Based on UserProfilesSettings.tsx with empty state, delete confirmation, unsaved badge.
struct UserProfilesEditor: View {
    @State private var profiles: [UserProfile] = []
    @State private var selectedId: String?
    @State private var editorContent = ""
    @State private var isDirty = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var isCreating = false
    @State private var newName = ""
    @State private var deleteConfirm = false
    @State private var error: String?
    @Environment(\.theme) private var theme

    private var selectedProfile: UserProfile? { profiles.first { $0.id == selectedId } }

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            // Left pane
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "User Profiles") { isCreating.toggle() }
                    .padding(.bottom, 12)

                if isCreating {
                    HStack(spacing: 6) {
                        BobeTextField(placeholder: "profile-name", text: $newName) {
                            if !newName.isEmpty { createProfile() }
                        }
                        Button("Create") { createProfile() }
                        .bobeButton(.primary, size: .small)
                        .disabled(newName.isEmpty)
                        Button { isCreating = false; newName = "" } label: {
                            Text("Cancel")
                        }
                        .bobeButton(.secondary, size: .small)
                    }
                }

                if isLoading && profiles.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading profiles...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if profiles.isEmpty && !isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "person.crop.circle")
                            .font(.system(size: 28))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("No profiles yet")
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(theme.colors.textMuted)
                        Text("Profiles tell BoBe about your background and preferences")
                            .bobeTextStyle(.helper)
                            .foregroundStyle(theme.colors.textMuted.opacity(0.7))
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(profiles) { profile in
                                BobeSelectableRow(
                                    isSelected: selectedId == profile.id,
                                    action: { selectedId = profile.id },
                                    content: {
                                    VStack(alignment: .leading) {
                                        HStack(spacing: 4) {
                                            Text(profile.name)
                                                .bobeTextStyle(.rowTitle)
                                            if profile.isDefault {
                                                Text("default")
                                                    .bobeTextStyle(.badge)
                                                    .padding(.horizontal, 4)
                                                    .padding(.vertical, 1)
                                                    .background(Capsule().fill(theme.colors.primary.opacity(0.2)))
                                                    .foregroundStyle(theme.colors.primary)
                                            }
                                        }
                                        Text(String(profile.content.prefix(60)).replacingOccurrences(of: "\n", with: " "))
                                            .bobeTextStyle(.rowMeta)
                                            .foregroundStyle(theme.colors.textMuted)
                                            .lineLimit(1)
                                    }
                                    Spacer()
                                    BobeToggle(isOn: Binding(
                                        get: { profile.enabled },
                                        set: { _ in toggleProfile(profile) }
                                    ))
                                })
                            }
                        }
                    }
                    .background(theme.colors.background)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
            .padding(.top, BobeMetrics.paneTopPadding)
        } right: {
            // Right pane
            if let profile = selectedProfile {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(profile.name)
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(theme.colors.text)
                        if isDirty {
                            Text("unsaved")
                                .bobeTextStyle(.badge)
                                .foregroundStyle(theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(theme.colors.tertiary.opacity(0.15)))
                        }
                        if profile.isDefault {
                            Text("default")
                                .bobeTextStyle(.badge)
                                .foregroundStyle(theme.colors.primary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(theme.colors.primary.opacity(0.15)))
                        }
                        Spacer()

                        if !profile.isDefault {
                            if deleteConfirm {
                                HStack(spacing: 6) {
                                    Text("Delete?")
                                        .font(.system(size: 12))
                                        .foregroundStyle(theme.colors.primary)
                                    Button("Yes") { deleteProfile(profile); deleteConfirm = false }
                                        .bobeButton(.destructive, size: .small)
                                    Button("No") { deleteConfirm = false }
                                        .bobeButton(.secondary, size: .small)
                                }
                            } else {
                                Button { deleteConfirm = true } label: {
                                    Image(systemName: "trash")
                                }
                                .bobeButton(.destructive, size: .small)
                            }
                        }

                        if isDirty {
                            Button("Discard") { discardChanges() }
                                .bobeButton(.secondary, size: .small)
                        }
                        Button(isSaving ? "Saving..." : "Save") { saveProfile() }
                            .bobeButton(.primary, size: .small)
                            .disabled(!isDirty || isSaving)
                    }

                    CodeEditor(text: $editorContent, theme: theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(theme.colors.surface)
                                .stroke(theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: editorContent) { _, _ in
                            isDirty = editorContent != selectedProfile?.content
                        }

                    if let error {
                        Text(error).font(.caption).foregroundStyle(theme.colors.primary)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
                .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
                .padding(.top, BobeMetrics.paneTopPadding)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "person.crop.circle")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a profile to edit")
                        .bobeTextStyle(.rowTitle)
                        .foregroundStyle(theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: selectedId) { _, newId in
            if let profile = profiles.first(where: { $0.id == newId }) {
                editorContent = profile.content
                isDirty = false
                deleteConfirm = false
            }
        }
        .task { await loadProfiles() }
    }

    // MARK: - Actions

    private func loadProfiles() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listUserProfiles()
            profiles = resp.profiles
            if selectedId == nil { selectedId = profiles.first?.id }
        } catch { self.error = error.localizedDescription }
    }

    private func createProfile() {
        Task {
            do {
                let profile = try await DaemonClient.shared.createUserProfile(
                    UserProfileCreateRequest(name: newName.lowercased(), content: "# \(newName)\n\n")
                )
                profiles.append(profile)
                selectedId = profile.id
                newName = ""
                isCreating = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func saveProfile() {
        guard let id = selectedId else { return }
        isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateUserProfile(id, UserProfileUpdateRequest(content: editorContent))
                if let idx = profiles.firstIndex(where: { $0.id == id }) { profiles[idx] = updated }
                isDirty = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func deleteProfile(_ profile: UserProfile) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteUserProfile(profile.id)
                profiles.removeAll { $0.id == profile.id }
                if selectedId == profile.id { selectedId = profiles.first?.id }
            } catch { self.error = error.localizedDescription }
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
                await loadProfiles()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func discardChanges() {
        if let profile = selectedProfile {
            editorContent = profile.content
            isDirty = false
        }
    }
}

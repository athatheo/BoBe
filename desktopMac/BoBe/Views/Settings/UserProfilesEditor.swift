import SwiftUI

/// Split-pane editor for User Profiles.
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

    private var selectedProfile: UserProfile? {
        self.profiles.first { $0.id == self.selectedId }
    }

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "User Profiles") { self.isCreating.toggle() }
                    .padding(.bottom, 12)

                if self.isCreating {
                    HStack(spacing: 6) {
                        BobeTextField(placeholder: "profile-name", text: self.$newName) {
                            if !self.newName.isEmpty { self.createProfile() }
                        }
                        Button("Create") { self.createProfile() }
                            .bobeButton(.primary, size: .small)
                            .disabled(self.newName.isEmpty)
                        Button {
                            self.isCreating = false
                            self.newName = ""
                        } label: {
                            Text("Cancel")
                        }
                        .bobeButton(.secondary, size: .small)
                    }
                }

                if self.isLoading, self.profiles.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading profiles...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.profiles.isEmpty, !self.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "person.crop.circle")
                            .font(.system(size: 28))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("No profiles yet")
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("Profiles tell BoBe about your background and preferences")
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.textMuted.opacity(0.7))
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    List {
                        ForEach(self.profiles) { profile in
                            BobeSelectableRow(
                                isSelected: self.selectedId == profile.id,
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
                                        accessibilityLabel: "Enable profile \(profile.name)"
                                    )
                                }
                            )
                            .onTapGesture { self.selectedId = profile.id }
                            .listRowSeparator(.hidden)
                            .listRowBackground(Color.clear)
                            .listRowInsets(EdgeInsets(top: 2, leading: 0, bottom: 2, trailing: 0))
                        }
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .background(self.theme.colors.background)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
            .padding(.top, BobeMetrics.paneTopPadding)
        } right: {
            if let profile = selectedProfile {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(profile.name)
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.text)
                        if self.isDirty {
                            Text("unsaved")
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.tertiary.opacity(0.15)))
                        }
                        if profile.isDefault {
                            Text("default")
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.primary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.primary.opacity(0.15)))
                        }
                        Spacer()

                        if !profile.isDefault {
                            if self.deleteConfirm {
                                HStack(spacing: 6) {
                                    Text("Delete?")
                                        .font(.system(size: 12))
                                        .foregroundStyle(self.theme.colors.primary)
                                    Button("Yes") {
                                        self.deleteProfile(profile)
                                        self.deleteConfirm = false
                                    }
                                    .bobeButton(.destructive, size: .small)
                                    Button("No") { self.deleteConfirm = false }
                                        .bobeButton(.secondary, size: .small)
                                }
                            } else {
                                Button {
                                    self.deleteConfirm = true
                                } label: {
                                    Image(systemName: "trash")
                                }
                                .accessibilityLabel("Delete profile")
                                .bobeButton(.destructive, size: .small)
                            }
                        }

                        if self.isDirty {
                            Button("Discard") { self.discardChanges() }
                                .bobeButton(.secondary, size: .small)
                        }
                        Button(self.isSaving ? "Saving..." : "Save") { self.saveProfile() }
                            .bobeButton(.primary, size: .small)
                            .disabled(!self.isDirty || self.isSaving)
                    }

                    CodeEditor(text: self.$editorContent, theme: self.theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: self.editorContent) { _, _ in
                            self.isDirty = self.editorContent != self.selectedProfile?.content
                        }

                    if let error {
                        Text(error).font(.caption).foregroundStyle(self.theme.colors.primary)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
                .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
                .padding(.top, BobeMetrics.paneTopPadding)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "person.crop.circle")
                        .font(.system(size: 28))
                        .foregroundStyle(self.theme.colors.textMuted)
                    Text("Select a profile to edit")
                        .bobeTextStyle(.rowTitle)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: self.selectedId) { _, newId in
            if let profile = profiles.first(where: { $0.id == newId }) {
                self.editorContent = profile.content
                self.isDirty = false
                self.deleteConfirm = false
            }
        }
        .task { await self.loadProfiles() }
    }

    // MARK: - Actions

    private func loadProfiles() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listUserProfiles()
            self.profiles = resp.profiles
            if self.selectedId == nil { self.selectedId = self.profiles.first?.id }
        } catch { self.error = error.localizedDescription }
    }

    private func createProfile() {
        Task {
            do {
                let profile = try await DaemonClient.shared.createUserProfile(
                    UserProfileCreateRequest(name: self.newName.lowercased(), content: "# \(self.newName)\n\n")
                )
                self.profiles.append(profile)
                self.selectedId = profile.id
                self.newName = ""
                self.isCreating = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func saveProfile() {
        guard let id = selectedId else { return }
        self.isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateUserProfile(id, UserProfileUpdateRequest(content: self.editorContent))
                if let idx = profiles.firstIndex(where: { $0.id == id }) { self.profiles[idx] = updated }
                self.isDirty = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func deleteProfile(_ profile: UserProfile) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteUserProfile(profile.id)
                self.profiles.removeAll { $0.id == profile.id }
                if self.selectedId == profile.id { self.selectedId = self.profiles.first?.id }
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
                await self.loadProfiles()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func discardChanges() {
        if let profile = selectedProfile {
            self.editorContent = profile.content
            self.isDirty = false
        }
    }
}

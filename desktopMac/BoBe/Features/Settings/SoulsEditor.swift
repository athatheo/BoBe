import SwiftUI

/// Split-pane editor for Souls (personality documents).
struct SoulsEditor: View {
    @State private var souls: [Soul] = []
    @State private var editorState = SettingsEditorState<String>()
    @State private var editorContent = ""
    @State private var newName = ""
    @Environment(\.theme) private var theme

    private var selectedSoul: Soul? {
        self.souls.first { $0.id == self.editorState.selectedId }
    }

    var body: some View {
        SettingsEditorScaffold(hasSelection: self.selectedSoul != nil) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "Souls") { self.editorState.isCreating.toggle() }
                    .padding(.bottom, 12)

                if self.editorState.isCreating {
                    HStack(spacing: 6) {
                        BobeTextField(placeholder: "soul-name", text: self.$newName) {
                            if !self.newName.isEmpty { self.createSoul() }
                        }
                        Button("Create") { self.createSoul() }
                            .bobeButton(.primary, size: .small)
                            .disabled(self.newName.isEmpty)
                        Button {
                            self.editorState.setCreating(false)
                            self.newName = ""
                        } label: {
                            Text("Cancel")
                        }
                        .bobeButton(.secondary, size: .small)
                    }
                }

                if self.editorState.isLoading, self.souls.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading souls...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.souls.isEmpty, !self.editorState.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "sparkles")
                            .font(.system(size: 28))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("No souls yet")
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("Create one to define BoBe's personality")
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(self.souls) { soul in
                                BobeSelectableRow(
                                    isSelected: self.editorState.selectedId == soul.id,
                                    content: {
                                        VStack(alignment: .leading) {
                                            HStack(spacing: 4) {
                                                Text(soul.name)
                                                    .bobeTextStyle(.rowTitle)
                                                if soul.isDefault {
                                                    Text("default")
                                                        .bobeTextStyle(.badge)
                                                        .padding(.horizontal, 4)
                                                        .padding(.vertical, 1)
                                                        .background(Capsule().fill(self.theme.colors.primary.opacity(0.2)))
                                                        .foregroundStyle(self.theme.colors.primary)
                                                }
                                            }
                                            Text(String(soul.content.prefix(60)).replacingOccurrences(of: "\n", with: " "))
                                                .bobeTextStyle(.rowMeta)
                                                .foregroundStyle(self.theme.colors.textMuted)
                                                .lineLimit(1)
                                        }
                                        Spacer()
                                        BobeToggle(
                                            isOn: Binding(
                                                get: { soul.enabled },
                                                set: { _ in self.toggleSoul(soul) }
                                            ),
                                            accessibilityLabel: "Enable soul"
                                        )
                                    }
                                )
                                .onTapGesture { self.editorState.select(soul.id) }
                            }
                        }
                    }
                    .background(self.theme.colors.background)
                }
            }
        } detailPane: {
            if let soul = self.selectedSoul {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(soul.name)
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.text)
                        if self.editorState.isDirty {
                            Text("unsaved")
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.tertiary.opacity(0.15)))
                        }
                        if soul.isDefault {
                            Text("default")
                                .bobeTextStyle(.badge)
                                .foregroundStyle(self.theme.colors.primary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.primary.opacity(0.15)))
                        }
                        Spacer()

                        if !soul.isDefault {
                            if self.editorState.showDeleteConfirmation {
                                HStack(spacing: 6) {
                                    Text("Delete?")
                                        .font(.system(size: 12))
                                        .foregroundStyle(self.theme.colors.primary)
                                    Button("Yes") {
                                        self.deleteSoul(soul)
                                        self.editorState.dismissDeleteConfirmation()
                                    }
                                    .bobeButton(.destructive, size: .small)
                                    Button("No") { self.editorState.dismissDeleteConfirmation() }
                                        .bobeButton(.secondary, size: .small)
                                }
                            } else {
                                Button {
                                    self.editorState.requestDeleteConfirmation()
                                } label: {
                                    Image(systemName: "trash")
                                }
                                .accessibilityLabel("Delete soul")
                                .bobeButton(.destructive, size: .small)
                            }
                        }

                        SettingsEditorSaveActions(
                            isDirty: self.editorState.isDirty,
                            isSaving: self.editorState.isSaving,
                            onDiscard: self.discardChanges,
                            onSave: self.saveSoul
                        )
                    }

                    CodeEditor(text: self.$editorContent, theme: self.theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: self.editorContent) { _, _ in
                            self.editorState.setDirty(self.editorContent != self.selectedSoul?.content)
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
                Image(systemName: "sparkles")
                    .font(.system(size: 28))
                    .foregroundStyle(self.theme.colors.textMuted)
                Text("Select a soul to edit")
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
        .onChange(of: self.editorState.selectedId) { _, newId in
            if let soul = self.souls.first(where: { $0.id == newId }) {
                self.editorContent = soul.content
                self.editorState.setDirty(false)
                self.editorState.dismissDeleteConfirmation()
            }
        }
        .task { await self.loadSouls() }
    }

    // MARK: - Actions

    private func loadSouls() async {
        self.editorState.setLoading(true)
        defer { self.editorState.setLoading(false) }
        do {
            let resp = try await DaemonClient.shared.listSouls()
            self.souls = resp.souls
            if self.editorState.selectedId == nil {
                self.editorState.select(self.souls.first?.id)
            }
        } catch {
            self.editorState.setError(error)
        }
    }

    private func createSoul() {
        Task {
            do {
                let soul = try await DaemonClient.shared.createSoul(
                    SoulCreateRequest(name: self.newName.lowercased(), content: "# \(self.newName)\n\n")
                )
                self.souls.append(soul)
                self.editorState.select(soul.id)
                self.newName = ""
                self.editorState.setCreating(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func saveSoul() {
        guard let id = self.editorState.selectedId else { return }
        self.editorState.setSaving(true)
        Task {
            defer { self.editorState.setSaving(false) }
            do {
                let updated = try await DaemonClient.shared.updateSoul(id, SoulUpdateRequest(content: self.editorContent))
                if let idx = self.souls.firstIndex(where: { $0.id == id }) {
                    self.souls[idx] = updated
                }
                self.editorState.setDirty(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func deleteSoul(_ soul: Soul) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteSoul(soul.id)
                self.souls.removeAll { $0.id == soul.id }
                if self.editorState.selectedId == soul.id {
                    self.editorState.select(self.souls.first?.id)
                }
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func toggleSoul(_ soul: Soul) {
        Task {
            do {
                if soul.enabled {
                    _ = try await DaemonClient.shared.disableSoul(soul.id)
                } else {
                    _ = try await DaemonClient.shared.enableSoul(soul.id)
                }
                await self.loadSouls()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func discardChanges() {
        if let soul = self.selectedSoul {
            self.editorContent = soul.content
            self.editorState.setDirty(false)
        }
    }
}

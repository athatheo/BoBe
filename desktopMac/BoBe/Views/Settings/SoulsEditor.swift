import SwiftUI

/// Split-pane editor for Souls (personality documents).
struct SoulsEditor: View {
    @State private var souls: [Soul] = []
    @State private var selectedId: String?
    @State private var editorContent = ""
    @State private var isDirty = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var newName = ""
    @State private var isCreating = false
    @State private var deleteConfirm = false
    @State private var error: String?
    @Environment(\.theme) private var theme

    private var selectedSoul: Soul? {
        self.souls.first { $0.id == self.selectedId }
    }

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "Souls") { self.isCreating.toggle() }
                    .padding(.bottom, 12)

                if self.isCreating {
                    HStack(spacing: 6) {
                        BobeTextField(placeholder: "soul-name", text: self.$newName) {
                            if !self.newName.isEmpty { self.createSoul() }
                        }
                        Button("Create") { self.createSoul() }
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

                if self.isLoading, self.souls.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading souls...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.souls.isEmpty, !self.isLoading {
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
                                    isSelected: self.selectedId == soul.id,
                                    action: { self.selectedId = soul.id },
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
                                            )
                                        )
                                    }
                                )
                            }
                        }
                    }
                    .background(self.theme.colors.background)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
            .padding(.top, BobeMetrics.paneTopPadding)
        } right: {
            if let soul = selectedSoul {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(soul.name)
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
                            if self.deleteConfirm {
                                HStack(spacing: 6) {
                                    Text("Delete?")
                                        .font(.system(size: 12))
                                        .foregroundStyle(self.theme.colors.primary)
                                    Button("Yes") {
                                        self.deleteSoul(soul)
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
                                .bobeButton(.destructive, size: .small)
                            }
                        }

                        if self.isDirty {
                            Button("Discard") { self.discardChanges() }
                                .bobeButton(.secondary, size: .small)
                        }
                        Button(self.isSaving ? "Saving..." : "Save") { self.saveSoul() }
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
                            self.isDirty = self.editorContent != self.selectedSoul?.content
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
                    Image(systemName: "sparkles")
                        .font(.system(size: 28))
                        .foregroundStyle(self.theme.colors.textMuted)
                    Text("Select a soul to edit")
                        .bobeTextStyle(.rowTitle)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: self.selectedId) { _, newId in
            if let soul = souls.first(where: { $0.id == newId }) {
                self.editorContent = soul.content
                self.isDirty = false
                self.deleteConfirm = false
            }
        }
        .task { await self.loadSouls() }
    }

    // MARK: - Actions

    private func loadSouls() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listSouls()
            self.souls = resp.souls
            if self.selectedId == nil { self.selectedId = self.souls.first?.id }
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func createSoul() {
        Task {
            do {
                let soul = try await DaemonClient.shared.createSoul(
                    SoulCreateRequest(name: self.newName.lowercased(), content: "# \(self.newName)\n\n")
                )
                self.souls.append(soul)
                self.selectedId = soul.id
                self.newName = ""
                self.isCreating = false
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func saveSoul() {
        guard let id = selectedId else { return }
        self.isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateSoul(id, SoulUpdateRequest(content: self.editorContent))
                if let idx = souls.firstIndex(where: { $0.id == id }) {
                    self.souls[idx] = updated
                }
                self.isDirty = false
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func deleteSoul(_ soul: Soul) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteSoul(soul.id)
                self.souls.removeAll { $0.id == soul.id }
                if self.selectedId == soul.id { self.selectedId = self.souls.first?.id }
            } catch {
                self.error = error.localizedDescription
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
                self.error = error.localizedDescription
            }
        }
    }

    private func discardChanges() {
        if let soul = selectedSoul {
            self.editorContent = soul.content
            self.isDirty = false
        }
    }
}

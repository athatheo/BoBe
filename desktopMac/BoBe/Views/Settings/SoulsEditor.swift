import SwiftUI

/// Split-pane editor for Souls (personality documents).
/// Based on SoulsSettings.tsx with empty state, delete confirmation, unsaved badge.
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

    private var selectedSoul: Soul? { souls.first { $0.id == selectedId } }

    var body: some View {
        HSplitView {
            // Left pane — list
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Souls")
                        .font(.headline)
                        .foregroundStyle(theme.colors.text)
                    Spacer()
                    Button { isCreating.toggle() } label: {
                        Image(systemName: "plus.circle.fill")
                    }
                    .buttonStyle(.plain)
                }

                if isCreating {
                    HStack(spacing: 6) {
                        TextField("soul-name", text: $newName)
                            .textFieldStyle(.roundedBorder)
                            .onSubmit { if !newName.isEmpty { createSoul() } }
                        Button { createSoul() } label: {
                            Image(systemName: "checkmark")
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(newName.isEmpty)
                        Button { isCreating = false; newName = "" } label: {
                            Image(systemName: "xmark")
                        }
                        .buttonStyle(.plain)
                    }
                }

                if isLoading && souls.isEmpty {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading souls...")
                            .font(.system(size: 12))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if souls.isEmpty && !isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "sparkles")
                            .font(.system(size: 28))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("No souls yet")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("Create one to define BoBe's personality")
                            .font(.system(size: 11))
                            .foregroundStyle(theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    List(selection: $selectedId) {
                        ForEach(souls) { soul in
                            HStack {
                                VStack(alignment: .leading) {
                                    HStack(spacing: 4) {
                                        Text(soul.name).font(.system(size: 13, weight: .medium))
                                        if soul.isDefault {
                                            Text("default")
                                                .font(.system(size: 9))
                                                .padding(.horizontal, 4)
                                                .padding(.vertical, 1)
                                                .background(Capsule().fill(theme.colors.primary.opacity(0.2)))
                                                .foregroundStyle(theme.colors.primary)
                                        }
                                    }
                                    Text(String(soul.content.prefix(60)).replacingOccurrences(of: "\n", with: " "))
                                        .font(.system(size: 10))
                                        .foregroundStyle(theme.colors.textMuted)
                                        .lineLimit(1)
                                }
                                Spacer()
                                BobeToggle(isOn: Binding(
                                    get: { soul.enabled },
                                    set: { _ in toggleSoul(soul) }
                                ))
                            }
                            .tag(soul.id)
                            .listRowBackground(theme.colors.background)
                        }
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .background(theme.colors.background)
                }
            }
            .frame(minWidth: 200, idealWidth: 280)
            .padding(12)

            // Right pane — editor
            if let soul = selectedSoul {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(soul.name)
                            .font(.headline)
                            .foregroundStyle(theme.colors.text)
                        if isDirty {
                            Text("unsaved")
                                .font(.system(size: 9, weight: .medium))
                                .foregroundStyle(.orange)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(.orange.opacity(0.15)))
                        }
                        if soul.isDefault {
                            Text("default")
                                .font(.system(size: 9, weight: .medium))
                                .foregroundStyle(theme.colors.primary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(theme.colors.primary.opacity(0.15)))
                        }
                        Spacer()

                        if !soul.isDefault {
                            if deleteConfirm {
                                HStack(spacing: 6) {
                                    Text("Delete?")
                                        .font(.system(size: 12))
                                        .foregroundStyle(.red)
                                    Button("Yes") { deleteSoul(soul); deleteConfirm = false }
                                        .buttonStyle(.bordered)
                                        .controlSize(.small)
                                        .tint(.red)
                                    Button("No") { deleteConfirm = false }
                                        .buttonStyle(.bordered)
                                        .controlSize(.small)
                                }
                            } else {
                                Button { deleteConfirm = true } label: {
                                    Image(systemName: "trash")
                                }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                                .tint(.red)
                            }
                        }

                        if isDirty {
                            Button("Discard") { discardChanges() }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                        }
                        Button(isSaving ? "Saving..." : "Save") { saveSoul() }
                            .buttonStyle(.borderedProminent)
                            .tint(theme.colors.primary)
                            .controlSize(.small)
                            .disabled(!isDirty || isSaving)
                    }

                    CodeEditor(text: $editorContent, theme: theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(theme.colors.surface)
                                .stroke(theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: editorContent) { _, _ in
                            isDirty = editorContent != selectedSoul?.content
                        }

                    if let error {
                        Text(error).font(.caption).foregroundStyle(.red)
                    }
                }
                .padding(12)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sparkles")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a soul to edit")
                        .font(.system(size: 13))
                        .foregroundStyle(theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: selectedId) { _, newId in
            if let soul = souls.first(where: { $0.id == newId }) {
                editorContent = soul.content
                isDirty = false
                deleteConfirm = false
            }
        }
        .task { await loadSouls() }
    }

    // MARK: - Actions

    private func loadSouls() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listSouls()
            souls = resp.souls
            if selectedId == nil { selectedId = souls.first?.id }
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func createSoul() {
        Task {
            do {
                let soul = try await DaemonClient.shared.createSoul(
                    SoulCreateRequest(name: newName.lowercased(), content: "# \(newName)\n\n")
                )
                souls.append(soul)
                selectedId = soul.id
                newName = ""
                isCreating = false
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func saveSoul() {
        guard let id = selectedId else { return }
        isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateSoul(id, SoulUpdateRequest(content: editorContent))
                if let idx = souls.firstIndex(where: { $0.id == id }) {
                    souls[idx] = updated
                }
                isDirty = false
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func deleteSoul(_ soul: Soul) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteSoul(soul.id)
                souls.removeAll { $0.id == soul.id }
                if selectedId == soul.id { selectedId = souls.first?.id }
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
                await loadSouls()
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func discardChanges() {
        if let soul = selectedSoul {
            editorContent = soul.content
            isDirty = false
        }
    }
}

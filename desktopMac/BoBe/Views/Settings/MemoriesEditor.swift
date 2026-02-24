import SwiftUI

/// Split-pane editor for Memories with category/type filters.
/// Based on MemoriesSettings.tsx with filters, empty state, delete confirmation.
struct MemoriesEditor: View {
    @State private var memories: [Memory] = []
    @State private var selectedId: String?
    @State private var editorContent = ""
    @State private var selectedCategory: MemoryCategory = .general
    @State private var isDirty = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var isCreating = false
    @State private var newContent = ""
    @State private var filterCategory: MemoryCategory?
    @State private var filterType: MemoryType?
    @State private var deleteConfirm = false
    @State private var error: String?
    @Environment(\.theme) private var theme

    private var selectedMemory: Memory? { memories.first { $0.id == selectedId } }

    private var filteredMemories: [Memory] {
        memories.filter { m in
            (filterCategory == nil || m.category == filterCategory) &&
            (filterType == nil || m.memoryType == filterType)
        }
    }

    private let categoryLabels: [MemoryCategory: String] = [
        .general: "GEN", .preference: "PRF", .pattern: "PAT",
        .fact: "FCT", .interest: "INT", .observation: "OBS"
    ]

    private let typeLabels: [MemoryType: String] = [
        .shortTerm: "ST", .longTerm: "LT", .explicit: "EX"
    ]

    var body: some View {
        HSplitView {
            // Left pane
            VStack(alignment: .leading, spacing: 8) {
                // Header with filters
                HStack(spacing: 6) {
                    Picker("", selection: $filterCategory) {
                        Text("All").tag(MemoryCategory?.none)
                        ForEach(MemoryCategory.allCases, id: \.self) { cat in
                            Text(cat.rawValue.capitalized).tag(MemoryCategory?.some(cat))
                        }
                    }
                    .pickerStyle(.menu)
                    .controlSize(.small)
                    .frame(maxWidth: 100)

                    Picker("", selection: $filterType) {
                        Text("All").tag(MemoryType?.none)
                        Text("Short").tag(MemoryType?.some(.shortTerm))
                        Text("Long").tag(MemoryType?.some(.longTerm))
                        Text("Explicit").tag(MemoryType?.some(.explicit))
                    }
                    .pickerStyle(.menu)
                    .controlSize(.small)
                    .frame(maxWidth: 80)

                    Spacer()

                    Text("\(filteredMemories.count)")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(theme.colors.textMuted)

                    Button { isCreating.toggle() } label: {
                        Image(systemName: "plus")
                            .font(.system(size: 12))
                    }
                    .buttonStyle(.plain)
                }

                if let error {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.circle.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(.red)
                        Text(error)
                            .font(.system(size: 10))
                            .foregroundStyle(.red)
                            .lineLimit(1)
                        Spacer()
                        Button("Retry") { Task { await loadMemories() } }
                            .buttonStyle(.bordered)
                            .controlSize(.mini)
                    }
                }

                if isCreating {
                    VStack(spacing: 6) {
                        TextEditor(text: $newContent)
                            .font(.system(size: 12))
                            .frame(height: 50)
                            .scrollContentBackground(.hidden)
                            .padding(4)
                            .background(RoundedRectangle(cornerRadius: 6).stroke(theme.colors.border))
                        HStack(spacing: 6) {
                            Button("Create") { createMemory() }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                                .disabled(newContent.isEmpty)
                            Button("Cancel") { isCreating = false; newContent = "" }
                                .buttonStyle(.plain)
                                .controlSize(.small)
                        }
                    }
                    .transition(.opacity.combined(with: .move(edge: .top)))
                }

                if isLoading && memories.isEmpty {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading memories...")
                            .font(.system(size: 12))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if filteredMemories.isEmpty && !isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "brain.head.profile")
                            .font(.system(size: 28))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("No memories")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    List(selection: $selectedId) {
                        ForEach(filteredMemories) { memory in
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(String(memory.content.prefix(45)))
                                        .font(.system(size: 12, weight: .medium))
                                        .lineLimit(1)
                                    HStack(spacing: 4) {
                                        Text(categoryLabels[memory.category] ?? memory.category.rawValue)
                                            .font(.system(size: 9, weight: .medium))
                                        Text(typeLabels[memory.memoryType] ?? memory.memoryType.rawValue)
                                            .font(.system(size: 9, weight: .medium))
                                    }
                                    .foregroundStyle(theme.colors.textMuted)
                                }
                                .opacity(memory.enabled ? 1 : 0.4)
                                Spacer()
                                BobeToggle(isOn: Binding(
                                    get: { memory.enabled },
                                    set: { _ in toggleMemory(memory) }
                                ))
                            }
                            .tag(memory.id)
                        }
                    }
                    .listStyle(.bordered)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .padding(12)

            // Right pane
            if let memory = selectedMemory {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 6) {
                        Text("Memory")
                            .font(.system(size: 11, weight: .medium))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(theme.colors.border.opacity(0.4)))

                        if isDirty {
                            Text("unsaved")
                                .font(.system(size: 9, weight: .medium))
                                .foregroundStyle(.orange)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(.orange.opacity(0.15)))
                        }

                        Text(categoryLabels[memory.category] ?? memory.category.rawValue)
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(theme.colors.primary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(theme.colors.primary.opacity(0.15)))

                        Text(typeLabels[memory.memoryType] ?? memory.memoryType.rawValue)
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(theme.colors.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(theme.colors.secondary.opacity(0.15)))

                        Spacer()

                        Picker("", selection: $selectedCategory) {
                            ForEach(MemoryCategory.allCases, id: \.self) { cat in
                                Text(cat.rawValue.capitalized).tag(cat)
                            }
                        }
                        .pickerStyle(.menu)
                        .controlSize(.small)
                        .frame(width: 130)
                        .onChange(of: selectedCategory) { _, _ in isDirty = true }

                        Text(memory.createdAt.prefix(10))
                            .font(.system(size: 10))
                            .foregroundStyle(theme.colors.textMuted)
                    }

                    TextEditor(text: $editorContent)
                        .font(.system(size: 13, design: .monospaced))
                        .scrollContentBackground(.hidden)
                        .padding(8)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(theme.colors.surface)
                                .stroke(theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: editorContent) { _, _ in
                            isDirty = editorContent != selectedMemory?.content || selectedCategory != selectedMemory?.category
                        }

                    HStack(spacing: 8) {
                        if deleteConfirm {
                            HStack(spacing: 6) {
                                Text("Delete?")
                                    .font(.system(size: 12))
                                    .foregroundStyle(.red)
                                Button("Yes") { deleteMemory(memory); deleteConfirm = false }
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

                        Spacer()

                        if isDirty {
                            Button("Discard") { discardChanges() }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                        }
                        Button(isSaving ? "Saving..." : "Save") { saveMemory() }
                            .buttonStyle(.borderedProminent)
                            .tint(theme.colors.primary)
                            .controlSize(.small)
                            .disabled(!isDirty || isSaving)
                    }
                }
                .padding(12)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "brain.head.profile")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a memory to edit")
                        .font(.system(size: 13))
                        .foregroundStyle(theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: selectedId) { _, newId in
            if let memory = memories.first(where: { $0.id == newId }) {
                editorContent = memory.content
                selectedCategory = memory.category
                isDirty = false
                deleteConfirm = false
            }
        }
        .task { await loadMemories() }
    }

    // MARK: - Actions

    private func loadMemories() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listMemories()
            memories = resp.memories
            if selectedId == nil { selectedId = memories.first?.id }
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    private func createMemory() {
        Task {
            do {
                let memory = try await DaemonClient.shared.createMemory(MemoryCreateRequest(content: newContent))
                memories.append(memory)
                selectedId = memory.id
                newContent = ""
                isCreating = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func saveMemory() {
        guard let id = selectedId else { return }
        isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateMemory(id, MemoryUpdateRequest(content: editorContent, category: selectedCategory))
                if let idx = memories.firstIndex(where: { $0.id == id }) { memories[idx] = updated }
                isDirty = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func deleteMemory(_ memory: Memory) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteMemory(memory.id)
                memories.removeAll { $0.id == memory.id }
                if selectedId == memory.id { selectedId = memories.first?.id }
            } catch { self.error = error.localizedDescription }
        }
    }

    private func toggleMemory(_ memory: Memory) {
        Task {
            do {
                if memory.enabled {
                    _ = try await DaemonClient.shared.disableMemory(memory.id)
                } else {
                    _ = try await DaemonClient.shared.enableMemory(memory.id)
                }
                await loadMemories()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func discardChanges() {
        if let memory = selectedMemory {
            editorContent = memory.content
            selectedCategory = memory.category
            isDirty = false
        }
    }
}

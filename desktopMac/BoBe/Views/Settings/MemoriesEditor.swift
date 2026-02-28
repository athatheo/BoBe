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
        .general: "General", .preference: "Preference", .pattern: "Pattern",
        .fact: "Fact", .interest: "Interest", .observation: "Observation"
    ]

    private let typeLabels: [MemoryType: String] = [
        .shortTerm: "Short-term", .longTerm: "Long-term", .explicit: "Explicit"
    ]

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            // Left pane
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "Memories") { isCreating.toggle() }
                    .padding(.bottom, 12)

                // Header with filters
                HStack(spacing: 6) {
                    BobeMenuPicker(
                        selection: $filterCategory,
                        options: [MemoryCategory?.none] + MemoryCategory.allCases.map { .some($0) },
                        label: { selected in
                            if let selected { return selected.rawValue.capitalized }
                            return "All"
                        },
                        width: 110,
                        size: .small
                    )

                    BobeMenuPicker(
                        selection: $filterType,
                        options: [MemoryType?.none, .some(.shortTerm), .some(.longTerm), .some(.explicit)],
                        label: { selected in
                            switch selected {
                            case .shortTerm?: "Short"
                            case .longTerm?: "Long"
                            case .explicit?: "Explicit"
                            default: "All"
                            }
                        },
                        width: 90,
                        size: .small
                    )

                    Spacer()

                    Text("\(filteredMemories.count)")
                        .bobeTextStyle(.badge)
                        .foregroundStyle(theme.colors.textMuted)
                }

                if let error {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.circle.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(theme.colors.primary)
                        Text(error)
                            .bobeTextStyle(.badge)
                            .foregroundStyle(theme.colors.primary)
                            .lineLimit(1)
                        Spacer()
                        Button("Retry") { Task { await loadMemories() } }
                            .bobeButton(.secondary, size: .mini)
                    }
                }

                if isCreating {
                    VStack(spacing: 6) {
                        CodeEditor(text: $newContent, theme: theme, fontSize: 12)
                            .frame(height: 50)
                            .background(
                                RoundedRectangle(cornerRadius: 6)
                                    .fill(theme.colors.surface)
                                    .stroke(theme.colors.border, lineWidth: 1)
                            )
                        HStack(spacing: 6) {
                            Button("Create") { createMemory() }
                                .bobeButton(.primary, size: .small)
                                .disabled(newContent.isEmpty)
                            Button("Cancel") { isCreating = false; newContent = "" }
                                .bobeButton(.secondary, size: .small)
                        }
                    }
                    .transition(.opacity.combined(with: .move(edge: .top)))
                }

                if isLoading && memories.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading memories...")
                            .bobeTextStyle(.body)
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
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(filteredMemories) { memory in
                                BobeSelectableRow(
                                    isSelected: selectedId == memory.id,
                                    action: { selectedId = memory.id },
                                    content: {
                                    HStack {
                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(String(memory.content.prefix(45)))
                                                .bobeTextStyle(.rowTitle)
                                                .lineLimit(1)
                                            let catLabel = categoryLabels[memory.category] ?? memory.category.rawValue
                                            let typeLabel = typeLabels[memory.memoryType] ?? memory.memoryType.rawValue
                                            Text("\(catLabel) · \(typeLabel)")
                                                .bobeTextStyle(.rowMeta)
                                                .foregroundStyle(theme.colors.textMuted)
                                        }
                                        .opacity(memory.enabled ? 1 : 0.45)
                                        Spacer()
                                        BobeToggle(isOn: Binding(
                                            get: { memory.enabled },
                                            set: { _ in toggleMemory(memory) }
                                        ))
                                    }
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
                                .foregroundStyle(theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(theme.colors.tertiary.opacity(0.15)))
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

                        BobeMenuPicker(
                            selection: $selectedCategory,
                            options: MemoryCategory.allCases,
                            label: { $0.rawValue.capitalized },
                            width: 130,
                            size: .small
                        )
                        .onChange(of: selectedCategory) { _, _ in isDirty = true }

                        Text(memory.createdAt.prefix(10))
                            .bobeTextStyle(.badge)
                            .foregroundStyle(theme.colors.textMuted)
                    }

                    CodeEditor(text: $editorContent, theme: theme, fontSize: 13)
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
                                    .foregroundStyle(theme.colors.primary)
                                Button("Yes") { deleteMemory(memory); deleteConfirm = false }
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

                        Spacer()

                        if isDirty {
                            Button("Discard") { discardChanges() }
                                .bobeButton(.secondary, size: .small)
                        }
                        Button(isSaving ? "Saving..." : "Save") { saveMemory() }
                            .bobeButton(.primary, size: .small)
                            .disabled(!isDirty || isSaving)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
                .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
                .padding(.top, BobeMetrics.paneTopPadding)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "brain.head.profile")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a memory to edit")
                        .bobeTextStyle(.rowTitle)
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

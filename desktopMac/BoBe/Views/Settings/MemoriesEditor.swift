import SwiftUI

/// Split-pane editor for Memories with category/type filters.
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

    private var selectedMemory: Memory? {
        self.memories.first { $0.id == self.selectedId }
    }

    private var filteredMemories: [Memory] {
        self.memories.filter { m in
            (self.filterCategory == nil || m.category == self.filterCategory) && (self.filterType == nil || m.memoryType == self.filterType)
        }
    }

    private let categoryLabels: [MemoryCategory: String] = [
        .general: "General", .preference: "Preference", .pattern: "Pattern",
        .fact: "Fact", .interest: "Interest", .observation: "Observation",
    ]

    private let typeLabels: [MemoryType: String] = [
        .shortTerm: "Short-term", .longTerm: "Long-term", .explicit: "Explicit",
    ]

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "Memories") { self.isCreating.toggle() }
                    .padding(.bottom, 12)

                HStack(spacing: 6) {
                    BobeMenuPicker(
                        selection: self.$filterCategory,
                        options: [MemoryCategory?.none] + MemoryCategory.allCases.map { .some($0) },
                        label: { selected in
                            if let selected { return selected.rawValue.capitalized }
                            return "All"
                        },
                        width: 110,
                        size: .small
                    )
                    .accessibilityLabel("Filter memories by category")

                    BobeMenuPicker(
                        selection: self.$filterType,
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
                    .accessibilityLabel("Filter memories by type")

                    Spacer()

                    Text("\(self.filteredMemories.count)")
                        .bobeTextStyle(.badge)
                        .foregroundStyle(self.theme.colors.textMuted)
                }

                if let error {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.circle.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(self.theme.colors.primary)
                        Text(error)
                            .bobeTextStyle(.badge)
                            .foregroundStyle(self.theme.colors.primary)
                            .lineLimit(1)
                        Spacer()
                        Button("Retry") { Task { await self.loadMemories() } }
                            .bobeButton(.secondary, size: .mini)
                    }
                }

                if self.isCreating {
                    VStack(spacing: 6) {
                        CodeEditor(text: self.$newContent, theme: self.theme, fontSize: 12)
                            .frame(height: 50)
                            .background(
                                RoundedRectangle(cornerRadius: 6)
                                    .fill(self.theme.colors.surface)
                                    .stroke(self.theme.colors.border, lineWidth: 1)
                            )
                        HStack(spacing: 6) {
                            Button("Create") { self.createMemory() }
                                .bobeButton(.primary, size: .small)
                                .disabled(self.newContent.isEmpty)
                            Button("Cancel") {
                                self.isCreating = false
                                self.newContent = ""
                            }
                            .bobeButton(.secondary, size: .small)
                        }
                    }
                    .transition(.opacity.combined(with: .move(edge: .top)))
                }

                if self.isLoading, self.memories.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading memories...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.filteredMemories.isEmpty, !self.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "brain.head.profile")
                            .font(.system(size: 28))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("No memories")
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    List(selection: self.$selectedId) {
                        ForEach(self.filteredMemories) { memory in
                            BobeSelectableRow(
                                isSelected: self.selectedId == memory.id,
                                content: {
                                    HStack {
                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(String(memory.content.prefix(45)))
                                                .bobeTextStyle(.rowTitle)
                                                .lineLimit(1)
                                            let catLabel = self.categoryLabels[memory.category] ?? memory.category.rawValue
                                            let typeLabel = self.typeLabels[memory.memoryType] ?? memory.memoryType.rawValue
                                            Text("\(catLabel) · \(typeLabel)")
                                                .bobeTextStyle(.rowMeta)
                                                .foregroundStyle(self.theme.colors.textMuted)
                                        }
                                        .opacity(memory.enabled ? 1 : 0.45)
                                        Spacer()
                                        BobeToggle(
                                            isOn: Binding(
                                                get: { memory.enabled },
                                                set: { _ in self.toggleMemory(memory) }
                                            ),
                                            accessibilityLabel: "Enable memory \(String(memory.content.prefix(40)))"
                                        )
                                    }
                                }
                            )
                            .tag(memory.id)
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
            if let memory = selectedMemory {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 6) {
                        Text("Memory")
                            .font(.system(size: 11, weight: .medium))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(self.theme.colors.border.opacity(0.4)))

                        if self.isDirty {
                            Text("unsaved")
                                .font(.system(size: 9, weight: .medium))
                                .foregroundStyle(self.theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.tertiary.opacity(0.15)))
                        }

                        Text(self.categoryLabels[memory.category] ?? memory.category.rawValue)
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(self.theme.colors.primary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(self.theme.colors.primary.opacity(0.15)))

                        Text(self.typeLabels[memory.memoryType] ?? memory.memoryType.rawValue)
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(self.theme.colors.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(self.theme.colors.secondary.opacity(0.15)))

                        Spacer()

                        BobeMenuPicker(
                            selection: self.$selectedCategory,
                            options: MemoryCategory.allCases,
                            label: { $0.rawValue.capitalized },
                            width: 130,
                            size: .small
                        )
                        .accessibilityLabel("Memory category")
                        .onChange(of: self.selectedCategory) { _, _ in self.isDirty = true }

                        Text(memory.createdAt.prefix(10))
                            .bobeTextStyle(.badge)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }

                    CodeEditor(text: self.$editorContent, theme: self.theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: self.editorContent) { _, _ in
                            self.isDirty = self.editorContent != self.selectedMemory?.content || self.selectedCategory != self.selectedMemory?
                                .category
                        }

                    HStack(spacing: 8) {
                        if self.deleteConfirm {
                            HStack(spacing: 6) {
                                Text("Delete?")
                                    .font(.system(size: 12))
                                    .foregroundStyle(self.theme.colors.primary)
                                Button("Yes") {
                                    self.deleteMemory(memory)
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
                            .accessibilityLabel("Delete memory")
                            .bobeButton(.destructive, size: .small)
                        }

                        Spacer()

                        if self.isDirty {
                            Button("Discard") { self.discardChanges() }
                                .bobeButton(.secondary, size: .small)
                        }
                        Button(self.isSaving ? "Saving..." : "Save") { self.saveMemory() }
                            .bobeButton(.primary, size: .small)
                            .disabled(!self.isDirty || self.isSaving)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
                .padding(.horizontal, BobeMetrics.paneHorizontalPadding)
                .padding(.top, BobeMetrics.paneTopPadding)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "brain.head.profile")
                        .font(.system(size: 28))
                        .foregroundStyle(self.theme.colors.textMuted)
                    Text("Select a memory to edit")
                        .bobeTextStyle(.rowTitle)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: self.selectedId) { _, newId in
            if let memory = memories.first(where: { $0.id == newId }) {
                self.editorContent = memory.content
                self.selectedCategory = memory.category
                self.isDirty = false
                self.deleteConfirm = false
            }
        }
        .task { await self.loadMemories() }
    }

    // MARK: - Actions

    private func loadMemories() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listMemories()
            self.memories = resp.memories
            if self.selectedId == nil { self.selectedId = self.memories.first?.id }
            self.error = nil
        } catch { self.error = error.localizedDescription }
    }

    private func createMemory() {
        Task {
            do {
                let memory = try await DaemonClient.shared.createMemory(MemoryCreateRequest(content: self.newContent))
                self.memories.append(memory)
                self.selectedId = memory.id
                self.newContent = ""
                self.isCreating = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func saveMemory() {
        guard let id = selectedId else { return }
        self.isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateMemory(
                    id,
                    MemoryUpdateRequest(content: self.editorContent, category: self.selectedCategory)
                )
                if let idx = memories.firstIndex(where: { $0.id == id }) { self.memories[idx] = updated }
                self.isDirty = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func deleteMemory(_ memory: Memory) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteMemory(memory.id)
                self.memories.removeAll { $0.id == memory.id }
                if self.selectedId == memory.id { self.selectedId = self.memories.first?.id }
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
                await self.loadMemories()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func discardChanges() {
        if let memory = selectedMemory {
            self.editorContent = memory.content
            self.selectedCategory = memory.category
            self.isDirty = false
        }
    }
}

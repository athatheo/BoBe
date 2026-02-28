import SwiftUI

/// Split-pane editor for Goals with priority colors, status badges, and delete confirmation.
/// Based on GoalsSettings.tsx.
struct GoalsEditor: View {
    @State private var goals: [Goal] = []
    @State private var selectedId: String?
    @State private var editorContent = ""
    @State private var selectedPriority: GoalPriority = .medium
    @State private var isDirty = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var isCreating = false
    @State private var newContent = ""
    @State private var deleteConfirm = false
    @State private var error: String?
    @Environment(\.theme) private var theme

    private var selectedGoal: Goal? { goals.first { $0.id == selectedId } }

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            // Left pane
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "Goals") { isCreating.toggle() }
                    .padding(.bottom, 12)

                if isCreating {
                    VStack(spacing: 6) {
                        BobeTextField(placeholder: "What's the goal?", text: $newContent) {
                            if !newContent.isEmpty { createGoal() }
                        }
                        HStack(spacing: 6) {
                            Button("Create") { createGoal() }
                                .bobeButton(.primary, size: .small)
                                .disabled(newContent.isEmpty)
                            Button("Cancel") { isCreating = false; newContent = "" }
                                .bobeButton(.secondary, size: .small)
                        }
                    }
                }

                if isLoading && goals.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading goals...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if goals.isEmpty && !isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "target")
                            .font(.system(size: 28))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("No goals yet")
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(theme.colors.textMuted)
                        Text("Create goals for BoBe to work towards")
                            .bobeTextStyle(.helper)
                            .foregroundStyle(theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(goals) { goal in
                                BobeSelectableRow(
                                    isSelected: selectedId == goal.id,
                                    action: { selectedId = goal.id },
                                    content: {
                                    HStack(spacing: 8) {
                                        Circle()
                                            .fill(priorityColor(goal.priority))
                                            .frame(width: 8, height: 8)

                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(String(goal.content.prefix(40)))
                                                .bobeTextStyle(.rowTitle)
                                                .lineLimit(1)
                                            HStack(spacing: 4) {
                                                Text(goal.status.rawValue)
                                                Text("•")
                                                Text(goal.priority.rawValue)
                                                Text("•")
                                                Text(goal.source.rawValue)
                                            }
                                            .bobeTextStyle(.rowMeta)
                                            .foregroundStyle(theme.colors.textMuted)
                                        }
                                        Spacer()

                                        BobeToggle(isOn: Binding(
                                            get: { goal.enabled },
                                            set: { _ in toggleGoal(goal) }
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
            if let goal = selectedGoal {
                VStack(alignment: .leading, spacing: 8) {
                    // Header badges
                    HStack(spacing: 6) {
                        Text("Goal")
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

                        Text(goal.priority.rawValue)
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(priorityColor(goal.priority))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(priorityColor(goal.priority).opacity(0.15)))

                        Text(goal.status.rawValue.uppercased())
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(statusColor(goal.status))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(statusColor(goal.status).opacity(0.15)))

                        Spacer()
                    }

                    // Controls row
                    HStack(spacing: 8) {
                        BobeMenuPicker(
                            selection: $selectedPriority,
                            options: [GoalPriority.high, .medium, .low],
                            label: { priority in
                                switch priority {
                                case .high: "High Priority"
                                case .medium: "Medium Priority"
                                case .low: "Low Priority"
                                case .unknown: "Unknown"
                                }
                            },
                            width: 180
                        )
                        .onChange(of: selectedPriority) { _, _ in isDirty = true }

                        Spacer()

                        if goal.status == .active {
                            Button { completeGoal(goal) } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "checkmark")
                                    Text("Complete")
                                }
                            }
                            .bobeButton(.secondary, size: .small)

                            Button { archiveGoal(goal) } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "archivebox")
                                    Text("Archive")
                                }
                            }
                            .bobeButton(.secondary, size: .small)
                        }
                    }

                    // Editor
                    CodeEditor(text: $editorContent, theme: theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(theme.colors.surface)
                                .stroke(theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: editorContent) { _, _ in
                            isDirty = editorContent != selectedGoal?.content || selectedPriority != selectedGoal?.priority
                        }

                    // Toolbar
                    HStack(spacing: 8) {
                        if deleteConfirm {
                            HStack(spacing: 6) {
                                Text("Delete?")
                                    .font(.system(size: 12))
                                    .foregroundStyle(theme.colors.primary)
                                Button("Yes") { deleteGoal(goal); deleteConfirm = false }
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
                        Button(isSaving ? "Saving..." : "Save") { saveGoal() }
                            .bobeButton(.primary, size: .small)
                            .disabled(!isDirty || isSaving)
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
                    Image(systemName: "target")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a goal to edit")
                        .bobeTextStyle(.rowTitle)
                        .foregroundStyle(theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: selectedId) { _, newId in
            if let goal = goals.first(where: { $0.id == newId }) {
                editorContent = goal.content
                selectedPriority = goal.priority
                isDirty = false
                deleteConfirm = false
            }
        }
        .task { await loadGoals() }
    }

    // MARK: - Helpers

    private func priorityColor(_ priority: GoalPriority) -> Color {
        switch priority {
        case .high: theme.colors.primary
        case .medium: theme.colors.tertiary
        case .low: theme.colors.secondary
        case .unknown: theme.colors.textMuted
        }
    }

    private func statusColor(_ status: GoalStatus) -> Color {
        switch status {
        case .active: theme.colors.secondary
        case .completed: theme.colors.tertiary
        case .archived: theme.colors.textMuted
        case .unknown: theme.colors.textMuted
        }
    }

    // MARK: - Actions

    private func loadGoals() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listGoals()
            goals = resp.goals
            if selectedId == nil { selectedId = goals.first?.id }
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func createGoal() {
        Task {
            do {
                let goal = try await DaemonClient.shared.createGoal(GoalCreateRequest(content: newContent))
                goals.append(goal)
                selectedId = goal.id
                newContent = ""
                isCreating = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func saveGoal() {
        guard let id = selectedId else { return }
        isSaving = true
        Task {
            defer { isSaving = false }
            do {
                let updated = try await DaemonClient.shared.updateGoal(id, GoalUpdateRequest(content: editorContent, priority: selectedPriority))
                if let idx = goals.firstIndex(where: { $0.id == id }) { goals[idx] = updated }
                isDirty = false
            } catch { self.error = error.localizedDescription }
        }
    }

    private func deleteGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteGoal(goal.id)
                goals.removeAll { $0.id == goal.id }
                if selectedId == goal.id { selectedId = goals.first?.id }
            } catch { self.error = error.localizedDescription }
        }
    }

    private func completeGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.completeGoal(goal.id)
                await loadGoals()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func archiveGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.archiveGoal(goal.id)
                await loadGoals()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func toggleGoal(_ goal: Goal) {
        Task {
            do {
                let req = GoalUpdateRequest(enabled: !goal.enabled)
                _ = try await DaemonClient.shared.updateGoal(goal.id, req)
                await loadGoals()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func discardChanges() {
        if let goal = selectedGoal {
            editorContent = goal.content
            selectedPriority = goal.priority
            isDirty = false
        }
    }
}

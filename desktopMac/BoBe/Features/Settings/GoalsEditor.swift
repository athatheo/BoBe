import SwiftUI

/// Split-pane editor for Goals with priority colors, status badges, and delete confirmation.
struct GoalsEditor: View {
    @State private var goals: [Goal] = []
    @State private var editorState = SettingsEditorState<String>()
    @State private var editorContent = ""
    @State private var selectedPriority: GoalPriority = .medium
    @State private var newContent = ""
    @Environment(\.theme) private var theme

    private var selectedGoal: Goal? {
        self.goals.first { $0.id == self.editorState.selectedId }
    }

    var body: some View {
        SettingsEditorScaffold(hasSelection: self.selectedGoal != nil) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "Goals") { self.editorState.isCreating.toggle() }
                    .padding(.bottom, 12)

                if self.editorState.isCreating {
                    VStack(spacing: 6) {
                        BobeTextField(placeholder: "What's the goal?", text: self.$newContent) {
                            if !self.newContent.isEmpty { self.createGoal() }
                        }
                        HStack(spacing: 6) {
                            Button("Create") { self.createGoal() }
                                .bobeButton(.primary, size: .small)
                            .disabled(self.newContent.isEmpty)
                            Button("Cancel") {
                                self.editorState.setCreating(false)
                                self.newContent = ""
                            }
                            .bobeButton(.secondary, size: .small)
                        }
                    }
                }

                if self.editorState.isLoading, self.goals.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading goals...")
                            .bobeTextStyle(.body)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.goals.isEmpty, !self.editorState.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "target")
                            .font(.system(size: 28))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("No goals yet")
                            .bobeTextStyle(.rowTitle)
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("Create goals for BoBe to work towards")
                            .bobeTextStyle(.helper)
                            .foregroundStyle(self.theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(self.goals) { goal in
                                BobeSelectableRow(
                                    isSelected: self.editorState.selectedId == goal.id,
                                    action: { self.editorState.select(goal.id) },
                                    content: {
                                        HStack(spacing: 8) {
                                            Circle()
                                                .fill(self.priorityColor(goal.priority))
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
                                                .foregroundStyle(self.theme.colors.textMuted)
                                            }
                                            Spacer()

                                            BobeToggle(
                                                isOn: Binding(
                                                    get: { goal.enabled },
                                                    set: { _ in self.toggleGoal(goal) }
                                                )
                                            )
                                        }
                                    }
                                )
                            }
                        }
                    }
                    .background(self.theme.colors.background)
                }
            }
        } detailPane: {
            if let goal = self.selectedGoal {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 6) {
                        Text("Goal")
                            .font(.system(size: 11, weight: .medium))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(self.theme.colors.border.opacity(0.4)))

                        if self.editorState.isDirty {
                            Text("unsaved")
                                .font(.system(size: 9, weight: .medium))
                                .foregroundStyle(self.theme.colors.tertiary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(self.theme.colors.tertiary.opacity(0.15)))
                        }

                        Text(goal.priority.rawValue)
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(self.priorityColor(goal.priority))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(self.priorityColor(goal.priority).opacity(0.15)))

                        Text(goal.status.rawValue.uppercased())
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(self.statusColor(goal.status))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(self.statusColor(goal.status).opacity(0.15)))

                        Spacer()
                    }

                    HStack(spacing: 8) {
                        BobeMenuPicker(
                            selection: self.$selectedPriority,
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
                        .onChange(of: self.selectedPriority) { _, _ in
                            self.editorState.setDirty()
                        }

                        Spacer()

                        if goal.status == .active {
                            Button {
                                self.completeGoal(goal)
                            } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "checkmark")
                                    Text("Complete")
                                }
                            }
                            .bobeButton(.secondary, size: .small)

                            Button {
                                self.archiveGoal(goal)
                            } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "archivebox")
                                    Text("Archive")
                                }
                            }
                            .bobeButton(.secondary, size: .small)
                        }
                    }

                    CodeEditor(text: self.$editorContent, theme: self.theme, fontSize: 13)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )
                        .onChange(of: self.editorContent) { _, _ in
                            self.editorState.setDirty(
                                self.editorContent != self.selectedGoal?.content || self.selectedPriority != self.selectedGoal?.priority
                            )
                        }

                    SettingsEditorActionRow {
                        if self.editorState.showDeleteConfirmation {
                            HStack(spacing: 6) {
                                Text("Delete?")
                                    .font(.system(size: 12))
                                    .foregroundStyle(self.theme.colors.primary)
                                Button("Yes") {
                                    self.deleteGoal(goal)
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
                            .bobeButton(.destructive, size: .small)
                        }
                    } trailing: {
                        SettingsEditorSaveActions(
                            isDirty: self.editorState.isDirty,
                            isSaving: self.editorState.isSaving,
                            onDiscard: self.discardChanges,
                            onSave: self.saveGoal
                        )
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
                Image(systemName: "target")
                    .font(.system(size: 28))
                    .foregroundStyle(self.theme.colors.textMuted)
                Text("Select a goal to edit")
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
        .onChange(of: self.editorState.selectedId) { _, newId in
            if let goal = self.goals.first(where: { $0.id == newId }) {
                self.editorContent = goal.content
                self.selectedPriority = goal.priority
                self.editorState.setDirty(false)
                self.editorState.dismissDeleteConfirmation()
            }
        }
        .task { await self.loadGoals() }
    }

    // MARK: - Helpers

    private func priorityColor(_ priority: GoalPriority) -> Color {
        switch priority {
        case .high: self.theme.colors.primary
        case .medium: self.theme.colors.tertiary
        case .low: self.theme.colors.secondary
        case .unknown: self.theme.colors.textMuted
        }
    }

    private func statusColor(_ status: GoalStatus) -> Color {
        switch status {
        case .active: self.theme.colors.secondary
        case .paused: self.theme.colors.primary
        case .completed: self.theme.colors.tertiary
        case .archived: self.theme.colors.textMuted
        case .unknown: self.theme.colors.textMuted
        }
    }

    // MARK: - Actions

    private func loadGoals() async {
        self.editorState.setLoading(true)
        defer { self.editorState.setLoading(false) }
        do {
            let resp = try await DaemonClient.shared.listGoals()
            self.goals = resp.goals
            if self.editorState.selectedId == nil {
                self.editorState.select(self.goals.first?.id)
            }
        } catch {
            self.editorState.setError(error)
        }
    }

    private func createGoal() {
        Task {
            do {
                let goal = try await DaemonClient.shared.createGoal(GoalCreateRequest(content: self.newContent))
                self.goals.append(goal)
                self.editorState.select(goal.id)
                self.newContent = ""
                self.editorState.setCreating(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func saveGoal() {
        guard let id = self.editorState.selectedId else { return }
        self.editorState.setSaving(true)
        Task {
            defer { self.editorState.setSaving(false) }
            do {
                let updated = try await DaemonClient.shared.updateGoal(
                    id,
                    GoalUpdateRequest(content: self.editorContent, priority: self.selectedPriority)
                )
                if let idx = self.goals.firstIndex(where: { $0.id == id }) { self.goals[idx] = updated }
                self.editorState.setDirty(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func deleteGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.deleteGoal(goal.id)
                self.goals.removeAll { $0.id == goal.id }
                if self.editorState.selectedId == goal.id {
                    self.editorState.select(self.goals.first?.id)
                }
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func completeGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.completeGoal(goal.id)
                await self.loadGoals()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func archiveGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.archiveGoal(goal.id)
                await self.loadGoals()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func toggleGoal(_ goal: Goal) {
        Task {
            do {
                let req = GoalUpdateRequest(enabled: !goal.enabled)
                _ = try await DaemonClient.shared.updateGoal(goal.id, req)
                await self.loadGoals()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func discardChanges() {
        if let goal = self.selectedGoal {
            self.editorContent = goal.content
            self.selectedPriority = goal.priority
            self.editorState.setDirty(false)
        }
    }
}

import SwiftUI

/// Split-pane editor for Goals with priority colors, status badges, and delete confirmation.
/// Matches Electron GoalsSettings.tsx.
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
        HSplitView {
            // Left pane
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Goals")
                        .font(.headline)
                        .foregroundStyle(theme.colors.text)
                    Spacer()
                    Button { isCreating.toggle() } label: {
                        Image(systemName: "plus.circle.fill")
                    }
                    .buttonStyle(.plain)
                }

                if isCreating {
                    VStack(spacing: 6) {
                        TextField("What's the goal?", text: $newContent)
                            .textFieldStyle(.roundedBorder)
                            .onSubmit { if !newContent.isEmpty { createGoal() } }
                        HStack(spacing: 6) {
                            Button("Create") { createGoal() }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                                .disabled(newContent.isEmpty)
                            Button("Cancel") { isCreating = false; newContent = "" }
                                .buttonStyle(.plain)
                                .controlSize(.small)
                        }
                    }
                }

                if isLoading && goals.isEmpty {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading goals...")
                            .font(.system(size: 12))
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
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("Create goals for BoBe to work towards")
                            .font(.system(size: 11))
                            .foregroundStyle(theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    List(selection: $selectedId) {
                        ForEach(goals) { goal in
                            HStack(spacing: 8) {
                                Circle()
                                    .fill(priorityColor(goal.priority))
                                    .frame(width: 8, height: 8)

                                VStack(alignment: .leading) {
                                    Text(String(goal.content.prefix(40)))
                                        .font(.system(size: 12, weight: .medium))
                                        .lineLimit(1)
                                    HStack(spacing: 4) {
                                        Text(goal.status.rawValue)
                                        Text("•")
                                        Text(goal.priority.rawValue)
                                        Text("•")
                                        Text(goal.source.rawValue)
                                    }
                                    .font(.system(size: 9))
                                    .foregroundStyle(theme.colors.textMuted)
                                }

                                Spacer()

                                BobeToggle(isOn: Binding(
                                    get: { goal.enabled },
                                    set: { _ in toggleGoal(goal) }
                                ))
                            }
                            .tag(goal.id)
                        }
                    }
                    .listStyle(.bordered)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .padding(12)

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
                                .foregroundStyle(.orange)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(.orange.opacity(0.15)))
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
                        Picker("Priority", selection: $selectedPriority) {
                            Text("High Priority").tag(GoalPriority.high)
                            Text("Medium Priority").tag(GoalPriority.medium)
                            Text("Low Priority").tag(GoalPriority.low)
                        }
                        .pickerStyle(.menu)
                        .frame(width: 180)
                        .onChange(of: selectedPriority) { _, _ in isDirty = true }

                        Spacer()

                        if goal.status == .active {
                            Button { completeGoal(goal) } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "checkmark")
                                    Text("Complete")
                                }
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)

                            Button { archiveGoal(goal) } label: {
                                HStack(spacing: 4) {
                                    Image(systemName: "archivebox")
                                    Text("Archive")
                                }
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                        }
                    }

                    // Editor
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
                            isDirty = editorContent != selectedGoal?.content || selectedPriority != selectedGoal?.priority
                        }

                    // Toolbar
                    HStack(spacing: 8) {
                        if deleteConfirm {
                            HStack(spacing: 6) {
                                Text("Delete?")
                                    .font(.system(size: 12))
                                    .foregroundStyle(.red)
                                Button("Yes") { deleteGoal(goal); deleteConfirm = false }
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
                        Button(isSaving ? "Saving..." : "Save") { saveGoal() }
                            .buttonStyle(.borderedProminent)
                            .tint(theme.colors.primary)
                            .controlSize(.small)
                            .disabled(!isDirty || isSaving)
                    }

                    if let error {
                        Text(error).font(.caption).foregroundStyle(.red)
                    }
                }
                .padding(12)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "target")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a goal to edit")
                        .font(.system(size: 13))
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
        case .medium: Color(hex: "C8B090")
        case .low: theme.colors.secondary
        }
    }

    private func statusColor(_ status: GoalStatus) -> Color {
        switch status {
        case .active: theme.colors.secondary
        case .completed: .green
        case .archived: theme.colors.textMuted
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

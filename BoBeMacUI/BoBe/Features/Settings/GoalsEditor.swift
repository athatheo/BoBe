import SwiftUI

/// User-editable subset of a Goal. The four AI-curated sections
/// (`how_working_on_it`, `patterns_observed`, `attitude_feelings`,
/// `open_questions`) are read-only via the API — the chat agent
/// edits them on disk via SDK file ops.
private struct GoalDraft: Equatable {
    var title = ""
    var summary = ""
    var whyItMatters = ""
    var notes = ""
    var priority = 2
    var status: GoalStatus = .active

    init() {}

    init(_ goal: Goal) {
        self.title = goal.title
        self.summary = goal.summary
        self.whyItMatters = goal.whyItMatters
        self.notes = goal.notes
        self.priority = goal.priority
        self.status = goal.status
    }

    func equals(_ goal: Goal) -> Bool {
        self.title == goal.title
            && self.summary == goal.summary
            && self.whyItMatters == goal.whyItMatters
            && self.notes == goal.notes
            && self.priority == goal.priority
            && self.status == goal.status
    }
}

struct GoalsEditor: View {
    @State private var goals: [Goal] = []
    @State private var editorState = SettingsEditorState<String>()
    @State private var draft = GoalDraft()
    @State private var showArchived = false
    @State private var statusFilter: GoalStatus?
    @State private var newTitle = ""
    @State private var newPriority = 2
    @Environment(\.theme) private var theme

    private static let priorityRange = 0 ... 5
    private static let priorityOptions = Array(GoalsEditor.priorityRange)
    private static let statusFilterOptions: [GoalStatus?] = [nil, .active, .paused, .completed, .archived]
    private static let statusEditOptions: [GoalStatus] = [.active, .paused, .completed, .archived]

    private var selectedGoal: Goal? {
        self.goals.first { $0.id == self.editorState.selectedId }
    }

    var body: some View {
        SettingsEditorScaffold(hasSelection: self.selectedGoal != nil) {
            self.listPane
        } detailPane: {
            self.detailPane
        } emptyPane: {
            VStack(spacing: 8) {
                Image(systemName: "target")
                    .font(.system(size: 28))
                    .foregroundStyle(self.theme.colors.textMuted)
                Text(L10n.tr("settings.goals.empty.select"))
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
        .onChange(of: self.editorState.selectedId) { _, newId in
            if let goal = self.goals.first(where: { $0.id == newId }) {
                self.draft = GoalDraft(goal)
                self.editorState.setDirty(false)
                self.editorState.dismissDeleteConfirmation()
            }
        }
        .onChange(of: self.draft) { _, newDraft in
            if let goal = self.selectedGoal {
                self.editorState.setDirty(!newDraft.equals(goal))
            }
        }
        .onChange(of: self.showArchived) { _, _ in
            Task { await self.loadGoals() }
        }
        .onChange(of: self.statusFilter) { _, _ in
            Task { await self.loadGoals() }
        }
        .task { await self.loadGoals() }
    }

    // MARK: - List pane

    private var listPane: some View {
        VStack(alignment: .leading, spacing: 8) {
            SettingsPaneHeader(title: L10n.tr("settings.goals.title")) {
                self.editorState.isCreating.toggle()
                if !self.editorState.isCreating {
                    self.newTitle = ""
                }
            }

            HStack(spacing: 8) {
                BobeMenuPicker(
                    selection: self.$statusFilter,
                    options: Self.statusFilterOptions,
                    label: { status in
                        guard let status else { return L10n.tr("settings.common.all") }
                        return Self.statusLabel(status)
                    },
                    width: 110,
                    size: .small
                )

                Toggle(L10n.tr("settings.goals.filter.show_archived"), isOn: self.$showArchived)
                    .toggleStyle(.checkbox)
                    .controlSize(.small)
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.textMuted)

                Spacer()

                Text("\(self.goals.count)")
                    .bobeTextStyle(.badge)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .padding(.bottom, 4)

            if self.editorState.isCreating {
                self.createForm
            }

            if let errorMessage = self.editorState.errorMessage {
                SettingsEditorErrorText(message: errorMessage)
            }

            self.listContent
        }
    }

    private var createForm: some View {
        VStack(alignment: .leading, spacing: 6) {
            BobeTextField(
                placeholder: L10n.tr("settings.goals.new.placeholder"),
                text: self.$newTitle
            ) {
                if self.canCreate { self.createGoal() }
            }

            HStack(spacing: 6) {
                BobeMenuPicker(
                    selection: self.$newPriority,
                    options: Self.priorityOptions,
                    label: { L10n.tr("settings.goals.priority.label_format", $0) },
                    width: 110,
                    size: .small
                )

                Spacer()

                Button(L10n.tr("settings.editor.action.create")) { self.createGoal() }
                    .bobeButton(.primary, size: .small)
                    .disabled(!self.canCreate)
                Button(L10n.tr("settings.editor.action.cancel")) {
                    self.editorState.setCreating(false)
                    self.newTitle = ""
                    self.newPriority = 2
                }
                .bobeButton(.secondary, size: .small)
            }
        }
        .padding(8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(self.theme.colors.surface)
                .stroke(self.theme.colors.border, lineWidth: 1)
        )
    }

    private var canCreate: Bool {
        self.newTitle.trimmingCharacters(in: .whitespaces).count >= 3
    }

    @ViewBuilder
    private var listContent: some View {
        if self.editorState.isLoading, self.goals.isEmpty {
            HStack(spacing: 8) {
                BobeSpinner(size: 14)
                Text(L10n.tr("settings.goals.loading"))
                    .bobeTextStyle(.body)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.top, 20)
        } else if self.goals.isEmpty {
            VStack(spacing: 8) {
                Image(systemName: "target")
                    .font(.system(size: 28))
                    .foregroundStyle(self.theme.colors.textMuted)
                Text(L10n.tr("settings.goals.empty.title"))
                    .bobeTextStyle(.rowTitle)
                    .foregroundStyle(self.theme.colors.textMuted)
                Text(L10n.tr("settings.goals.empty.description"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted.opacity(0.7))
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 32)
        } else {
            ScrollView {
                LazyVStack(spacing: 4) {
                    ForEach(self.goals) { goal in
                        self.goalRow(goal)
                    }
                }
            }
            .background(self.theme.colors.background)
        }
    }

    private func goalRow(_ goal: Goal) -> some View {
        BobeSelectableRow(isSelected: self.editorState.selectedId == goal.id) {
            HStack(spacing: 10) {
                Circle()
                    .fill(self.priorityColor(goal.priority))
                    .frame(width: 8, height: 8)

                VStack(alignment: .leading, spacing: 3) {
                    Text(goal.title.isEmpty ? L10n.tr("settings.common.unknown") : goal.title)
                        .bobeTextStyle(.rowTitle)
                        .lineLimit(1)
                    HStack(spacing: 4) {
                        Text(Self.statusLabel(goal.status))
                        Text("•")
                        Text(L10n.tr("settings.goals.priority.label_format", goal.priority))
                        Text("•")
                        Text(Self.relativeDate(goal.updatedAt))
                    }
                    .bobeTextStyle(.rowMeta)
                    .foregroundStyle(self.theme.colors.textMuted)
                }
                Spacer()
            }
        }
        .overlay {
            Button { self.editorState.select(goal.id) } label: { Color.clear }
                .buttonStyle(.plain)
        }
    }

    // MARK: - Detail pane

    @ViewBuilder
    private var detailPane: some View {
        if let goal = self.selectedGoal {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    self.detailHeader(goal)
                    self.statusActionRow(goal)

                    CollapsibleSection(
                        title: L10n.tr("settings.goals.section.title"),
                        icon: "textformat"
                    ) {
                        BobeTextField(
                            placeholder: L10n.tr("settings.goals.new.placeholder"),
                            text: self.$draft.title
                        )
                    }

                    CollapsibleSection(
                        title: L10n.tr("settings.goals.section.status"),
                        icon: "flag.fill"
                    ) {
                        BobeMenuPicker(
                            selection: self.$draft.status,
                            options: Self.statusEditOptions,
                            label: { Self.statusLabel($0) },
                            width: 200
                        )
                    }

                    CollapsibleSection(
                        title: L10n.tr("settings.goals.section.priority"),
                        icon: "exclamationmark.triangle.fill",
                        description: L10n.tr("settings.goals.priority.range_hint")
                    ) {
                        BobeMenuPicker(
                            selection: self.$draft.priority,
                            options: Self.priorityOptions,
                            label: { L10n.tr("settings.goals.priority.label_format", $0) },
                            width: 140
                        )
                    }

                    CollapsibleSection(
                        title: L10n.tr("settings.goals.section.summary"),
                        icon: "text.alignleft"
                    ) {
                        self.proseEditor(text: self.$draft.summary, minHeight: 80)
                    }

                    CollapsibleSection(
                        title: L10n.tr("settings.goals.section.why_it_matters"),
                        icon: "questionmark.circle.fill"
                    ) {
                        self.proseEditor(text: self.$draft.whyItMatters, minHeight: 100)
                    }

                    CollapsibleSection(
                        title: L10n.tr("settings.goals.section.notes"),
                        icon: "note.text"
                    ) {
                        self.proseEditor(text: self.$draft.notes, minHeight: 240)
                    }

                    self.aiCuratedSection(
                        title: L10n.tr("settings.goals.section.how_working_on_it"),
                        icon: "hammer.fill",
                        body: goal.howWorkingOnIt
                    )
                    self.aiCuratedSection(
                        title: L10n.tr("settings.goals.section.patterns_observed"),
                        icon: "waveform.path.ecg",
                        body: goal.patternsObserved
                    )
                    self.aiCuratedSection(
                        title: L10n.tr("settings.goals.section.attitude_feelings"),
                        icon: "heart.fill",
                        body: goal.attitudeFeelings
                    )
                    self.aiCuratedSection(
                        title: L10n.tr("settings.goals.section.open_questions"),
                        icon: "questionmark.bubble.fill",
                        body: goal.openQuestions
                    )

                    SettingsEditorActionRow {
                        self.deleteRow(goal)
                    } trailing: {
                        SettingsEditorSaveActions(
                            isDirty: self.editorState.isDirty,
                            isSaving: self.editorState.isSaving,
                            onDiscard: { self.draft = GoalDraft(goal) },
                            onSave: self.saveGoal
                        )
                    }

                    if let errorMessage = self.editorState.errorMessage {
                        SettingsEditorErrorText(message: errorMessage)
                    }
                }
                .padding(.bottom, 24)
            }
        }
    }

    private func detailHeader(_ goal: Goal) -> some View {
        HStack(spacing: 6) {
            Text(L10n.tr("settings.goals.badge.goal"))
                .font(.system(size: 11, weight: .medium))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Capsule().fill(self.theme.colors.border.opacity(0.4)))

            if self.editorState.isDirty {
                Text(L10n.tr("settings.editor.badge.unsaved"))
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(self.theme.colors.tertiary)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Capsule().fill(self.theme.colors.tertiary.opacity(0.15)))
            }

            Text(L10n.tr("settings.goals.priority.label_format", self.draft.priority))
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(self.priorityColor(self.draft.priority))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Capsule().fill(self.priorityColor(self.draft.priority).opacity(0.15)))

            Text(Self.statusLabel(self.draft.status).uppercased())
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(self.statusColor(self.draft.status))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Capsule().fill(self.statusColor(self.draft.status).opacity(0.15)))

            Spacer()

            Text(Self.relativeDate(goal.updatedAt))
                .bobeTextStyle(.badge)
                .foregroundStyle(self.theme.colors.textMuted)
        }
    }

    @ViewBuilder
    private func statusActionRow(_ goal: Goal) -> some View {
        HStack(spacing: 6) {
            Spacer()
            switch goal.status {
            case .active:
                self.statusButton(L10n.tr("settings.goals.action.pause"), icon: "pause.circle") {
                    self.changeStatus(goal, to: .paused)
                }
                self.statusButton(L10n.tr("settings.goals.action.complete"), icon: "checkmark.circle") {
                    self.completeGoal(goal)
                }
                self.statusButton(L10n.tr("settings.goals.action.archive"), icon: "archivebox") {
                    self.archiveGoal(goal)
                }
            case .paused:
                self.statusButton(L10n.tr("settings.goals.action.resume"), icon: "play.circle") {
                    self.changeStatus(goal, to: .active)
                }
                self.statusButton(L10n.tr("settings.goals.action.complete"), icon: "checkmark.circle") {
                    self.completeGoal(goal)
                }
                self.statusButton(L10n.tr("settings.goals.action.archive"), icon: "archivebox") {
                    self.archiveGoal(goal)
                }
            case .completed:
                self.statusButton(L10n.tr("settings.goals.action.reactivate"), icon: "arrow.uturn.backward.circle") {
                    self.changeStatus(goal, to: .active)
                }
            case .archived:
                self.statusButton(L10n.tr("settings.goals.action.restore"), icon: "tray.and.arrow.up") {
                    self.changeStatus(goal, to: .active)
                }
            case .unknown:
                EmptyView()
            }
        }
    }

    private func statusButton(_ title: String, icon: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: icon)
                Text(title)
            }
        }
        .bobeButton(.secondary, size: .small)
    }

    private func proseEditor(text: Binding<String>, minHeight: CGFloat) -> some View {
        CodeEditor(text: text, theme: self.theme, fontSize: 13)
            .frame(minHeight: minHeight)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.theme.colors.surface)
                    .stroke(self.theme.colors.border, lineWidth: 1)
            )
    }

    private func aiCuratedSection(title: String, icon: String, body: String) -> some View {
        CollapsibleSection(
            title: title,
            icon: icon,
            aiCuratedBadge: true,
            initiallyExpanded: false
        ) {
            if body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                Text(L10n.tr("settings.goals.section.ai_empty"))
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted.opacity(0.8))
                    .italic()
            } else {
                Text(body)
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.text)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
        }
    }

    private func deleteRow(_ goal: Goal) -> some View {
        Group {
            if self.editorState.showDeleteConfirmation {
                HStack(spacing: 6) {
                    Text(L10n.tr("settings.editor.delete.confirm"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.primary)
                    Button(L10n.tr("settings.editor.delete.yes")) {
                        self.deleteGoal(goal)
                    }
                    .bobeButton(.destructive, size: .small)
                    Button(L10n.tr("settings.editor.delete.no")) {
                        self.editorState.dismissDeleteConfirmation()
                    }
                    .bobeButton(.secondary, size: .small)
                }
            } else {
                Button {
                    self.editorState.requestDeleteConfirmation()
                } label: {
                    Image(systemName: "trash")
                }
                .accessibilityLabel(L10n.tr("settings.goals.delete.accessibility"))
                .bobeButton(.destructive, size: .small)
            }
        }
    }

    // MARK: - Color helpers

    private func priorityColor(_ priority: Int) -> Color {
        switch priority {
        case 0, 1: self.theme.colors.textMuted
        case 2: self.theme.colors.secondary
        case 3: self.theme.colors.tertiary
        default: self.theme.colors.primary
        }
    }

    private func statusColor(_ status: GoalStatus) -> Color {
        switch status {
        case .active: self.theme.colors.secondary
        case .paused: self.theme.colors.primary
        case .completed: self.theme.colors.tertiary
        case .archived, .unknown: self.theme.colors.textMuted
        }
    }

    private static func statusLabel(_ status: GoalStatus) -> String {
        switch status {
        case .active: L10n.tr("settings.goals.status.active")
        case .paused: L10n.tr("settings.goals.status.paused")
        case .completed: L10n.tr("settings.goals.status.completed")
        case .archived: L10n.tr("settings.goals.status.archived")
        case .unknown: L10n.tr("settings.common.unknown")
        }
    }

    /// `2026-05-09T14:23:01Z` → `May 9, 2026` (or just yyyy-mm-dd as fallback).
    private static func relativeDate(_ iso: String) -> String {
        let isoFormatter = ISO8601DateFormatter()
        isoFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = isoFormatter.date(from: iso) ?? ISO8601DateFormatter().date(from: iso) {
            let display = DateFormatter()
            display.dateStyle = .medium
            display.timeStyle = .none
            return display.string(from: date)
        }
        return String(iso.prefix(10))
    }

    // MARK: - Daemon ops

    private func loadGoals() async {
        self.editorState.setLoading(true)
        defer { self.editorState.setLoading(false) }
        do {
            let resp = try await DaemonClient.shared.listGoals(
                status: self.statusFilter,
                includeArchived: self.showArchived || self.statusFilter == .archived
            )
            self.goals = resp.goals
            if let selected = self.editorState.selectedId, !self.goals.contains(where: { $0.id == selected }) {
                self.editorState.select(self.goals.first?.id)
            } else if self.editorState.selectedId == nil {
                self.editorState.select(self.goals.first?.id)
            }
            self.editorState.clearError()
        } catch {
            self.editorState.setError(error)
        }
    }

    private func createGoal() {
        let title = self.newTitle.trimmingCharacters(in: .whitespaces)
        guard !title.isEmpty else { return }
        Task {
            do {
                var req = GoalCreateRequest(title: title)
                req.priority = self.newPriority
                let goal = try await DaemonClient.shared.createGoal(req)
                self.goals.insert(goal, at: 0)
                self.editorState.select(goal.id)
                self.newTitle = ""
                self.newPriority = 2
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
                var req = GoalUpdateRequest()
                req.title = self.draft.title
                req.status = self.draft.status
                req.priority = self.draft.priority
                req.summary = self.draft.summary
                req.whyItMatters = self.draft.whyItMatters
                req.notes = self.draft.notes
                let updated = try await DaemonClient.shared.updateGoal(id, req)
                if let idx = self.goals.firstIndex(where: { $0.id == id }) {
                    self.goals[idx] = updated
                }
                self.draft = GoalDraft(updated)
                self.editorState.setDirty(false)
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func deleteGoal(_ goal: Goal) {
        Task {
            do {
                try await DaemonClient.shared.deleteGoal(goal.id)
                self.goals.removeAll { $0.id == goal.id }
                if self.editorState.selectedId == goal.id {
                    self.editorState.select(self.goals.first?.id)
                }
                self.editorState.dismissDeleteConfirmation()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    private func changeStatus(_ goal: Goal, to status: GoalStatus) {
        Task {
            do {
                var req = GoalUpdateRequest()
                req.status = status
                let updated = try await DaemonClient.shared.updateGoal(goal.id, req)
                if let idx = self.goals.firstIndex(where: { $0.id == goal.id }) {
                    self.goals[idx] = updated
                }
                if self.editorState.selectedId == goal.id {
                    self.draft = GoalDraft(updated)
                    self.editorState.setDirty(false)
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
}

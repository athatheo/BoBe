import SwiftUI

/// Read-only on API: AI-curated sections (how/patterns/attitude/questions) are agent-written.
struct GoalDraft: Equatable {
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
    @State var goals: [Goal] = []
    @State var editorState = SettingsEditorState<String>()
    @State var draft = GoalDraft()
    @State var showArchived = false
    @State var statusFilter: GoalStatus?
    @State var newTitle = ""
    @State var newPriority = 2
    @Environment(\.theme) var theme

    static let priorityRange = 0 ... 5
    static let priorityOptions = Array(GoalsEditor.priorityRange)
    static let statusFilterOptions: [GoalStatus?] = [nil, .active, .paused, .completed, .archived]
    static let statusEditOptions: [GoalStatus] = [.active, .paused, .completed, .archived]

    var selectedGoal: Goal? {
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

    var listPane: some View {
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

    var createForm: some View {
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

    var canCreate: Bool {
        self.newTitle.trimmingCharacters(in: .whitespaces).count >= 3
    }

    @ViewBuilder
    var listContent: some View {
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

    func goalRow(_ goal: Goal) -> some View {
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

    // MARK: - Color + label helpers

    func priorityColor(_ priority: Int) -> Color {
        switch priority {
        case 0, 1: self.theme.colors.textMuted
        case 2: self.theme.colors.secondary
        case 3: self.theme.colors.tertiary
        default: self.theme.colors.primary
        }
    }

    func statusColor(_ status: GoalStatus) -> Color {
        switch status {
        case .active: self.theme.colors.secondary
        case .paused: self.theme.colors.primary
        case .completed: self.theme.colors.tertiary
        case .archived, .unknown: self.theme.colors.textMuted
        }
    }

    static func statusLabel(_ status: GoalStatus) -> String {
        switch status {
        case .active: L10n.tr("settings.goals.status.active")
        case .paused: L10n.tr("settings.goals.status.paused")
        case .completed: L10n.tr("settings.goals.status.completed")
        case .archived: L10n.tr("settings.goals.status.archived")
        case .unknown: L10n.tr("settings.common.unknown")
        }
    }

    static func relativeDate(_ iso: String) -> String {
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
}

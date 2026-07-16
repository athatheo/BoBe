import Foundation

extension GoalsEditor {
    func loadGoals() async {
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

    func createGoal() {
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

    func saveGoal() async -> Bool {
        guard let id = self.editorState.selectedId else { return false }
        self.editorState.setSaving(true)
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
                return true
            } catch {
                self.editorState.setError(error)
                return false
            }
    }

    func deleteGoal(_ goal: Goal) {
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

    func changeStatus(_ goal: Goal, to status: GoalStatus) {
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

    func completeGoal(_ goal: Goal) {
        Task {
            do {
                _ = try await DaemonClient.shared.completeGoal(goal.id)
                await self.loadGoals()
            } catch {
                self.editorState.setError(error)
            }
        }
    }

    func archiveGoal(_ goal: Goal) {
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

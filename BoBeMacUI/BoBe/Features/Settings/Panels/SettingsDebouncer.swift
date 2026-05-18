import Foundation

/// Coalesces rapid setting mutations into one persist call and schedules
/// the "Saved ✓" toast clear. Replaces the four near-identical saveTask /
/// savedToastTask pairs that previously lived inline in each panel. Owns
/// the Task lifecycle so panels never see `Task.sleep` or cancellation
/// bookkeeping.
@MainActor
@Observable
final class SettingsDebouncer {
    static let debounceSeconds: Double = 0.6
    static let toastSeconds: Double = 2.4

    private var saveTask: Task<Void, Never>?
    private var toastTask: Task<Void, Never>?

    /// Cancel any pending persist, then run `work` after `debounceSeconds`.
    /// If a later call arrives within the window, the in-flight one is
    /// cancelled before it fires — only the last touched value persists.
    func debounce(_ work: @escaping @MainActor () async -> Void) {
        self.saveTask?.cancel()
        self.saveTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(Self.debounceSeconds))
            guard !Task.isCancelled else { return }
            await work()
        }
    }

    /// Clear the "Saved ✓" toast after `toastSeconds`. Cancels any prior
    /// pending clear so back-to-back saves don't snap the toast off early.
    func scheduleToastClear(_ clear: @escaping @MainActor () -> Void) {
        self.toastTask?.cancel()
        self.toastTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(Self.toastSeconds))
            guard !Task.isCancelled else { return }
            clear()
        }
    }

    /// Cancel only the toast clear. Used on panel disappear so a pending
    /// "Saved ✓" doesn't fire on a torn-down view, while the in-flight
    /// persist (if any) is allowed to finish in the background.
    func cancelToast() {
        self.toastTask?.cancel()
        self.toastTask = nil
    }

    /// Cancel a pending debounced save. Used when the caller needs to
    /// persist immediately (e.g. a user-initiated mode flip) and doesn't
    /// want the queued debounce to fire a redundant PATCH afterwards.
    func cancelPendingSave() {
        self.saveTask?.cancel()
        self.saveTask = nil
    }

    /// Standard settings-PATCH flow shared across every panel:
    ///   1. Build a partial `SettingsUpdateRequest` via the `build` closure.
    ///   2. PATCH `/settings`.
    ///   3. On `persistFailed`, route to `setError`. On success, surface
    ///      `resp.message` via `setSaved` and schedule its auto-clear.
    ///   4. On thrown error, surface the localized description.
    ///   5. Fire optional `onSuccess` for panel-specific side-effects
    ///      (VoicePanel uses it to refresh the pipeline + post the config
    ///      change notification).
    func runPersist(
        setError: @escaping (String?) -> Void,
        setSaved: @escaping (String?) -> Void,
        onSuccess: (() async -> Void)? = nil,
        build: (inout SettingsUpdateRequest) -> Void
    ) async {
        var req = SettingsUpdateRequest()
        build(&req)
        do {
            let resp = try await DaemonClient.shared.updateSettings(req)
            if resp.persistFailed == true {
                setError(L10n.tr("settings.shared.action.persist_failed"))
                setSaved(nil)
            } else {
                setError(nil)
                setSaved(resp.message)
                self.scheduleToastClear { setSaved(nil) }
                if let onSuccess { await onSuccess() }
            }
        } catch {
            setError(error.localizedDescription)
            setSaved(nil)
        }
    }
}

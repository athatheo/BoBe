import Foundation
import Observation
import OSLog
import SwiftUI

private let logger = Logger(subsystem: "com.bobe.app", category: "SettingsStore")

/// Single source of truth for the daemon `DaemonSettings` blob across the
/// Settings UI. Replaces per-panel `@State var settings: DaemonSettings?`
/// + per-panel `/settings` GET on `.task` appear.
///
/// Before: Engine / Voice / Behavior panels each fetched `/settings`
/// independently on appear, mutated their own local copies, and PATCHed
/// back. Three GETs per Settings-window session; edits made in one panel
/// were invisible to others until re-visit.
///
/// After: panels read `store.settings`, mutate via `store.update(_:)`
/// (which routes through the shared `SettingsDebouncer`), and observe
/// `store.savedMessage` / `store.error`. One GET per session.
///
/// `@MainActor` because the panel call sites all run on main and the
/// `@Observable` tracking is keyed to main-actor observers.
@MainActor
@Observable
final class SettingsStore {
    static let shared = SettingsStore()

    /// Loaded daemon settings. `nil` until the first fetch resolves or after
    /// `markStale()`. Panels gate their content on this nil-check.
    private(set) var settings: DaemonSettings?

    /// Auth status fetched alongside the engine pane. Independent of the
    /// settings blob (separate `/auth/status` endpoint).
    private(set) var auth: AuthStatusResponse?
    private(set) var authError: String?

    /// Latest persist error (load error, PATCH error). Cleared by the next
    /// successful save. Panels render this as a banner.
    private(set) var error: String?

    /// "Saved ✓" toast text from the daemon's last successful PATCH. Auto-
    /// clears after `SettingsDebouncer.toastSeconds`.
    private(set) var savedMessage: String?

    /// Restart-required fields the daemon reported on the last PATCH. Union
    /// of (locally-shadowed restart fields) + (daemon-reported). Cleared by
    /// `clearRestartFields()` when the banner is dismissed.
    private(set) var restartFields: Set<String> = []

    private(set) var isLoading = false

    /// Dedup guard so concurrent `loadIfNeeded()` calls share a single
    /// in-flight fetch. View-appear stampedes (Engine → Voice → Behavior in
    /// quick succession) all await this same task.
    private var loadTask: Task<Void, Never>?

    /// Auth fetch dedup mirror of `loadTask`.
    private var authTask: Task<Void, Never>?

    /// Owns the `BackendService.stateStream` observer that re-fetches the
    /// settings blob after the daemon recovers from a crash. Started lazily
    /// the first time a panel calls `loadIfNeeded()`.
    private var backendObserverTask: Task<Void, Never>?

    private let debouncer = SettingsDebouncer()

    private init() {}

    // MARK: - Load

    /// First-time load or hard refresh. Idempotent — concurrent callers
    /// await the same fetch. The first invocation also wires up the daemon-
    /// recovery observer so subsequent crash → ready cycles refetch the
    /// blob automatically (in-memory state could have drifted during the
    /// crash window).
    func loadIfNeeded(force: Bool = false) async {
        self.startBackendObserverIfNeeded()
        if !force, self.settings != nil { return }
        if let existing = self.loadTask {
            await existing.value
            return
        }
        let task = Task<Void, Never> { @MainActor in
            self.isLoading = true
            defer { self.isLoading = false }
            do {
                self.settings = try await DaemonClient.shared.getSettings()
                self.error = nil
            } catch {
                self.error = error.localizedDescription
                logger.error("settings load failed: \(error.localizedDescription, privacy: .public)")
            }
        }
        self.loadTask = task
        await task.value
        self.loadTask = nil
    }

    /// Watch the daemon's lifecycle for crash-recovery transitions. When
    /// the daemon comes back to `.ready` after a `.crashed` (or `.fatal`
    /// resolved by user-restart), refetch the settings blob so panels
    /// don't render stale in-memory state. Idempotent — the observer is
    /// started once at first load and survives for the process lifetime.
    private func startBackendObserverIfNeeded() {
        guard self.backendObserverTask == nil else { return }
        self.backendObserverTask = Task { @MainActor [weak self] in
            var wasCrashed = false
            for await state in BackendService.shared.stateStream {
                guard !Task.isCancelled else { return }
                switch state {
                case .crashed, .fatal:
                    wasCrashed = true
                case .ready where wasCrashed:
                    wasCrashed = false
                    logger.info("daemon recovered — refreshing settings blob")
                    await self?.loadIfNeeded(force: true)
                case .ready, .starting, .stopped:
                    break
                }
            }
        }
    }

    func loadAuth() async {
        if let existing = self.authTask {
            await existing.value
            return
        }
        let task = Task<Void, Never> { @MainActor in
            do {
                self.auth = try await DaemonClient.shared.getAuthStatus()
                self.authError = nil
            } catch {
                self.auth = nil
                self.authError = error.localizedDescription
                logger.debug("auth load failed: \(error.localizedDescription, privacy: .public)")
            }
        }
        self.authTask = task
        await task.value
        self.authTask = nil
    }

    /// Drop the cached blob so the next `loadIfNeeded()` refetches. Used
    /// after a daemon restart or when the user explicitly hits Reload.
    func markStale() {
        self.settings = nil
    }

    // MARK: - Mutate

    /// Apply an in-memory mutation and schedule a debounced PATCH. Panels
    /// use this instead of mutating their own `@State` copies — the
    /// `@Observable` macro picks up the assignment and all observers
    /// (including other panels visible on a different sidebar entry once
    /// we navigate back) re-render.
    ///
    /// `touched` is an optional field key for the restart-required banner;
    /// pass the same snake_case string the daemon's `applied_fields` uses.
    func update(touched: String? = nil, _ mutate: (inout DaemonSettings) -> Void) {
        guard var current = self.settings else { return }
        mutate(&current)
        self.settings = current

        let touchedFields = touched.map { Set([$0]) } ?? []
        self.debouncer.debounce { [weak self] in
            await self?.persist(currentSettings: current, touched: touchedFields)
        }
    }

    /// Apply a mutation and persist immediately, bypassing the debounce.
    /// Used for user-initiated explicit actions (engine mode flip) where
    /// the registry should rebuild right now, not after 600ms.
    func updateImmediate(touched: String? = nil, _ mutate: (inout DaemonSettings) -> Void) async {
        guard var current = self.settings else { return }
        mutate(&current)
        self.settings = current

        self.debouncer.cancelPendingSave()
        let touchedFields = touched.map { Set([$0]) } ?? []
        await self.persist(currentSettings: current, touched: touchedFields)
    }

    private func persist(currentSettings: DaemonSettings, touched: Set<String>) async {
        await self.debouncer.runPersist(
            setError: { [weak self] in self?.error = $0 },
            setSaved: { [weak self] in self?.savedMessage = $0 },
            onSuccess: {
                VoicePipeline.shared.applySettings(currentSettings)
            },
            build: { req in
                // Snapshot every field — daemon ignores nils, so a "patch-all"
                // request is fine and avoids field-key bookkeeping at every
                // call site.
                req.captureEnabled = currentSettings.captureEnabled
                req.captureIntervalSeconds = currentSettings.captureIntervalSeconds
                req.checkinEnabled = currentSettings.checkinEnabled
                req.checkinTimes = currentSettings.checkinTimes
                req.checkinJitterMinutes = currentSettings.checkinJitterMinutes
                req.conversationInactivityTimeoutSeconds = currentSettings.conversationInactivityTimeoutSeconds
                req.conversationAutoCloseMinutes = currentSettings.conversationAutoCloseMinutes
                req.goalCheckIntervalSeconds = currentSettings.goalCheckIntervalSeconds
                req.mcpEnabled = currentSettings.mcpEnabled
                req.engine = currentSettings.engine
                req.providerBaseUrl = PatchField(currentSettings.providerBaseUrl)
                req.providerChatModel = PatchField(currentSettings.providerChatModel)
                req.providerBatchModel = PatchField(currentSettings.providerBatchModel)
                req.providerVisionModel = PatchField(currentSettings.providerVisionModel)
                req.providerChatReasoning = PatchField(currentSettings.providerChatReasoning)
                req.providerBatchReasoning = PatchField(currentSettings.providerBatchReasoning)
                req.providerVisionReasoning = PatchField(currentSettings.providerVisionReasoning)
                req.providerOffline = currentSettings.providerOffline
                req.voiceEnabled = currentSettings.voiceEnabled
                req.voicePersona = currentSettings.voicePersona
                req.voiceSpeed = currentSettings.voiceSpeed
                req.voiceSttLanguage = currentSettings.voiceSttLanguage
                req.voicePauseSensitivity = currentSettings.voicePauseSensitivity
                req.voiceShowPartialCaption = currentSettings.voiceShowPartialCaption
            }
        )

        // Fold the daemon's restart-required reply into the store-level
        // banner state. Field-touched tracking is local to the caller
        // (Behavior panel) — most other callers pass nothing.
        if !touched.isEmpty {
            self.restartFields.formUnion(touched.intersection(Self.deferToRestartFields))
        }
    }

    /// Daemon-reported restart-required fields appended after a successful
    /// persist. Called by panels that need to know which specific fields
    /// were applied (BehaviorPanel renders the banner; others can ignore).
    func registerDaemonRestartFields(_ fields: [String]) {
        self.restartFields.formUnion(fields)
    }

    /// Clear the restart-required set when the user dismisses the banner.
    func clearRestartFields() {
        self.restartFields.removeAll()
    }

    /// Fields the daemon captures at boot and won't re-read live. Locally
    /// tracked so the banner appears immediately on touch, not after the
    /// daemon's roundtrip. Mirrors the previous panel-local sets in
    /// BehaviorPanel + MCPServersPanel.
    static let deferToRestartFields: Set<String> = [
        "checkin_enabled",
        "checkin_times",
        "checkin_jitter_minutes",
    ]

    // MARK: - Bindings

    /// Build a `Binding` from a writable key path on the settings blob.
    /// Falls back to `fallback()` when settings haven't loaded yet — the
    /// panel renders the picker in its placeholder state.
    func binding<V>(
        _ keyPath: WritableKeyPath<DaemonSettings, V>,
        fallback: @autoclosure @escaping () -> V,
        touched: String? = nil
    ) -> Binding<V> {
        Binding(
            get: { self.settings?[keyPath: keyPath] ?? fallback() },
            set: { [self] newValue in
                self.update(touched: touched) { $0[keyPath: keyPath] = newValue }
            }
        )
    }

    /// Same as `binding(_:fallback:touched:)` but for optional string fields
    /// that the daemon treats as "unset" when empty. Empties → nil.
    func optionalBinding(
        _ keyPath: WritableKeyPath<DaemonSettings, String?>,
        fallback: @autoclosure @escaping () -> String,
        touched: String? = nil
    ) -> Binding<String> {
        Binding(
            get: { self.settings?[keyPath: keyPath] ?? fallback() },
            set: { [self] newValue in
                self.update(touched: touched) { $0[keyPath: keyPath] = newValue.isEmpty ? nil : newValue }
            }
        )
    }

    /// Convenience for `Double`-backed fields a panel renders as `Int` (e.g.
    /// `goalCheckIntervalSeconds`). Rounds on read, casts on write.
    func intBinding(
        _ keyPath: WritableKeyPath<DaemonSettings, Double>,
        fallback: @autoclosure @escaping () -> Int,
        touched: String? = nil
    ) -> Binding<Int> {
        Binding(
            get: { Int((self.settings?[keyPath: keyPath] ?? Double(fallback())).rounded()) },
            set: { [self] newValue in
                self.update(touched: touched) { $0[keyPath: keyPath] = Double(newValue) }
            }
        )
    }

    func durationMinutesBinding(
        _ keyPath: WritableKeyPath<DaemonSettings, Double>,
        fallbackSeconds: @autoclosure @escaping () -> Int,
        touched: String? = nil
    ) -> Binding<Int> {
        Binding(
            get: {
                let seconds = self.settings?[keyPath: keyPath] ?? Double(fallbackSeconds())
                return max(1, Int((seconds / 60).rounded()))
            },
            set: { [self] minutes in
                self.update(touched: touched) {
                    $0[keyPath: keyPath] = Double(minutes * 60)
                }
            }
        )
    }

    // MARK: - Lifecycle

    func flushPendingSave() async {
        await self.debouncer.flush()
    }

    /// Called by a panel's `.onDisappear` to drain any pending toast clear
    /// without dropping the in-flight persist. The persist task itself is
    /// owned by the debouncer and runs to completion regardless.
    func cancelToast() {
        self.debouncer.cancelToast()
    }

    /// Clear the saved/error chrome without persisting. Used when a panel
    /// wants to refresh from the daemon without surfacing the previous
    /// toast on re-appear.
    func clearChrome() {
        self.savedMessage = nil
        self.error = nil
    }
}

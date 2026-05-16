import SwiftUI

/// Voice settings — daemon-side defaults for the persona/speed used when the
/// Hello handshake doesn't override, plus the master enable toggle, plus the
/// voice-model install status & reinstall affordance.
///
/// All fields hot-swap (no daemon restart). Voice toggle takes effect at the
/// next `/voice/stream` connect. Persona/speed apply on the next turn.
struct VoicePanel: View {
    @State private var pipeline = VoicePipeline.shared
    @State private var settings: DaemonSettings?
    @State private var isLoading = false
    @State private var isReinstalling = false
    @State private var error: String?
    @State private var savedMessage: String?
    @State private var debouncer = SettingsDebouncer()
    @State private var statusPollTask: Task<Void, Never>?
    /// Tick that increments every 500ms while a FluidAudio download is in
    /// flight, forcing the parakeet card to recompute its observed-percent.
    @State private var sttProgressTick: Int = 0
    @State private var sttProgressTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    /// Install snapshot — pulled from the central `VoicePipeline` observable
    /// so the wizard, mic button, and settings card all see the same data.
    private var installStatus: VoiceInstallSnapshot? {
        self.pipeline.installSnapshot
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("settings.voice.title"))
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("settings.voice.description"))
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.textMuted)

                if let error {
                    self.errorBanner(error)
                }
                if let savedMessage {
                    self.savedToast(savedMessage)
                }

                if self.settings != nil {
                    self.engineSection
                    self.modelsSection
                } else if self.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.engine.loading"))
                            .font(.system(size: 13))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                } else {
                    // Daemon unreachable / settings fetch failed — surface
                    // an explicit empty state rather than rendering nothing.
                    HStack(spacing: 8) {
                        Image(systemName: "exclamationmark.circle")
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text(L10n.tr("settings.voice.empty.unreachable"))
                            .font(.system(size: 13))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                }
            }
            .padding(24)
        }
        .task {
            await self.loadAll()
        }
        .onDisappear {
            self.statusPollTask?.cancel()
            self.sttProgressTask?.cancel()
            // Let any in-flight persist drain in the background — closing
            // Settings mid-edit must not silently drop the last keystroke.
            self.debouncer.cancelToast()
        }
        .onChange(of: self.pipeline.sttStatus) { _, newValue in
            self.startSttProgressTickerIfNeeded(for: newValue)
        }
    }

    // MARK: - Sections

    private var engineSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.voice.section.engine"),
            icon: "waveform",
            description: L10n.tr("settings.voice.description")
        ) {
            VStack(alignment: .leading, spacing: 12) {
                SettingsRow(
                    label: L10n.tr("settings.voice.enabled"),
                    description: L10n.tr("settings.voice.enabled.description")
                ) {
                    BobeToggle(isOn: self.binding(\.voiceEnabled, fallback: true))
                }

                // Language picker — drives client-side engine selection.
                // English uses FluidAudio Parakeet EOU today; other languages
                // are visible but marked "coming soon" until Qwen3-ASR wiring
                // lands (per the language matrix in docs/voice-architecture.md).
                SettingsRow(
                    label: "Language",
                    description: "Primary speech-recognition language. English ships today; others are queued behind FluidAudio Qwen3-ASR support."
                ) {
                    BobeMenuPicker(
                        selection: self.binding(\.voiceSttLanguage, fallback: "en"),
                        options: VoiceLanguages.all,
                        label: VoiceLanguages.displayName(for:),
                        width: 280
                    )
                }

                SettingsRow(
                    label: "Pause sensitivity",
                    description: "How long the silence after you stop talking before BoBe decides the turn is over. "
                        + "Patient = wait longer; Tight = cut earlier."
                ) {
                    BobeMenuPicker(
                        selection: self.binding(\.voicePauseSensitivity, fallback: "balanced"),
                        options: ["tight", "balanced", "patient"],
                        label: { $0.capitalized },
                        width: 280
                    )
                }

                SettingsRow(
                    label: L10n.tr("settings.voice.persona"),
                    description: L10n.tr("settings.voice.persona.description")
                ) {
                    BobeMenuPicker(
                        selection: self.binding(\.voicePersona, fallback: "af_bella"),
                        options: KokoroVoices.allSlots,
                        label: KokoroVoices.displayName(for:),
                        width: 280
                    )
                }

                SettingsRow(
                    label: L10n.tr("settings.voice.speed"),
                    description: L10n.tr("settings.voice.speed.description")
                ) {
                    HStack(spacing: 12) {
                        Slider(
                            value: self.binding(\.voiceSpeed, fallback: 1.0),
                            in: 0.5...2.0,
                            step: 0.05
                        )
                        .frame(width: 220)
                        .accessibilityLabel(L10n.tr("settings.voice.speed"))
                        .accessibilityHint(L10n.tr("settings.voice.speed.description"))
                        .accessibilityValue(String(format: "%.2f×", Double(self.settings?.voiceSpeed ?? 1.0)))
                        Text(String(format: "%.2f×", Double(self.settings?.voiceSpeed ?? 1.0)))
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(self.theme.colors.textMuted)
                            .frame(width: 56, alignment: .trailing)
                            .accessibilityHidden(true)
                    }
                }

                SettingsRow(
                    label: "Show partial caption",
                    description: "Display the live partial transcript while you speak. Off if you find the streaming text distracting."
                ) {
                    BobeToggle(isOn: self.binding(\.voiceShowPartialCaption, fallback: true))
                }
            }
        }
    }

    private var modelsSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.voice.section.models"),
            icon: "internaldrive",
            description: self.modelsSummary
        ) {
            VStack(alignment: .leading, spacing: 14) {
                // Daemon-side TTS (Kokoro).
                VoiceModelCard(
                    name: "Kokoro v1.0 multilingual",
                    purpose: "Text-to-speech voice synthesis (what BoBe sounds like).",
                    sizeHint: "~340 MB",
                    location: "~/.bobe/models/kokoro-multi-lang-v1_0/",
                    status: self.kokoroStatus,
                    daemonProgress: self.installStatus?.models.first(where: { $0.kind == VoiceWire.modelKindTts })
                )

                // Client-side STT — English (FluidAudio Parakeet EOU).
                VoiceModelCard(
                    name: "FluidAudio Parakeet EOU (English)",
                    purpose: "Speech-to-text recognition with end-of-utterance detection (what BoBe hears when language = English).",
                    sizeHint: "~600 MB",
                    location: "~/Library/Application Support/FluidAudio/Models/parakeet-eou-streaming/",
                    status: self.parakeetStatus,
                    daemonProgress: nil
                )

                // Client-side STT — Mandarin (FluidAudio Qwen3-ASR + VAD).
                // Card is always visible so the user can see the install
                // weight before switching languages; only consumed when
                // `voice.stt_language` is non-English (Qwen3 covers them all).
                VoiceModelCard(
                    name: "FluidAudio Qwen3-ASR (Mandarin)",
                    purpose: "Multilingual speech-to-text plus a Silero VAD for end-of-utterance detection (used when language ≠ English).",
                    sizeHint: "~1.75 GB",
                    location: "~/Library/Application Support/FluidAudio/Models/qwen3-asr-0.6b-coreml/",
                    status: self.qwen3Status,
                    daemonProgress: nil
                )

                // Global reinstall — restarts both pipelines in parallel.
                HStack(spacing: 8) {
                    Button(self.isReinstalling
                        ? "Reinstalling…"
                        : "Reinstall all"
                    ) {
                        Task { await self.reinstall() }
                    }
                    .bobeButton(.primary, size: .small)
                    .disabled(self.isReinstalling)
                    .accessibilityLabel("Reinstall all voice models")
                    Text(self.isReinstalling
                        ? "Models are downloading — see the per-model rows above for progress."
                        : "Re-downloads voice models if you suspect a corrupted install."
                    )
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.textMuted)
                }
                .padding(.top, 4)
            }
        }
    }

    private var kokoroStatus: VoiceModelCard.Status {
        guard let progress = self.installStatus?.models.first(where: { $0.kind == VoiceWire.modelKindTts }) else {
            return .unknown
        }
        if self.installStatus?.installed.tts == true {
            return .installed
        }
        // Daemon's per-model status: "pending" / "downloading X/Y" / "complete" / "failed".
        if progress.status.starts(with: "downloading") || self.isReinstalling {
            return .downloading(
                bytesDownloaded: progress.bytesDownloaded,
                bytesTotal: progress.bytesTotal,
                percent: progress.percent
            )
        }
        if progress.status == "failed" {
            return .failed(progress.status)
        }
        return .missing
    }

    private var parakeetStatus: VoiceModelCard.Status {
        // Reading `sttProgressTick` forces re-render during downloads so
        // the observed-bytes percent updates live without depending on the
        // @Observable system to fire (which it won't — fs polling isn't
        // an observable signal).
        _ = self.sttProgressTick
        return self.cardStatus(for: FluidAudioModelPresence.self)
    }

    private var qwen3Status: VoiceModelCard.Status {
        _ = self.sttProgressTick
        // When the user has a non-English language selected, the pipeline's
        // sttStatus tracks Qwen3 directly; under English, the pipeline is
        // running Parakeet so we fall back to bare disk presence.
        let activeLanguage = self.settings?.voiceSttLanguage ?? "en"
        if activeLanguage != "en" {
            return self.cardStatus(for: FluidAudioQwen3ModelPresence.self)
        }
        return FluidAudioQwen3ModelPresence.isInstalled() ? .installed : .missing
    }

    private func cardStatus<P: FluidAudioPresence>(for presence: P.Type) -> VoiceModelCard.Status {
        switch self.pipeline.sttStatus {
        case .ready: return .installed
        case .downloading:
            return .downloading(
                bytesDownloaded: P.observedBytes(),
                bytesTotal: P.approximateTotalBytes,
                percent: P.observedPercent()
            )
        case .failed(let msg): return .failed(msg)
        case .notLoaded:
            return P.isInstalled() ? .installed : .missing
        }
    }

    /// Start (or stop) the 500ms tick that drives the FluidAudio
    /// download-progress recompute. Idempotent — starting twice is a no-op,
    /// the .notLoaded/.ready/.failed cases cancel the task.
    private func startSttProgressTickerIfNeeded(for status: VoicePipeline.SttStatus) {
        switch status {
        case .downloading:
            if self.sttProgressTask != nil { return }
            self.sttProgressTask = Task {
                while !Task.isCancelled {
                    try? await Task.sleep(nanoseconds: 500_000_000)
                    if Task.isCancelled { return }
                    self.sttProgressTick &+= 1
                }
            }
        case .ready, .failed, .notLoaded:
            self.sttProgressTask?.cancel()
            self.sttProgressTask = nil
        }
    }

    private var modelsSummary: String {
        switch self.pipeline.readiness {
        case .ready:
            return L10n.tr("settings.voice.installed_label")
        case .installing, .modelsMissing, .failed:
            // Partial when one side is on disk; missing otherwise. Surface
            // through underlying signals so the label is honest rather than
            // collapsing to a single "in progress" word.
            let daemonReady = self.installStatus?.installed.allPresent ?? false
            let sttReady = self.pipeline.sttStatus == .ready
            if daemonReady || sttReady {
                return L10n.tr("settings.voice.partial_label")
            }
            return L10n.tr("settings.voice.missing_label")
        case .disabledByUser, .permissionMissing, .preparing:
            return L10n.tr("settings.voice.missing_label")
        }
    }

    // MARK: - I/O

    private func loadAll() async {
        if self.settings == nil {
            self.isLoading = true
            do {
                self.settings = try await DaemonClient.shared.getSettings()
            } catch {
                self.error = error.localizedDescription
            }
            self.isLoading = false
        }
        await self.refreshInstallStatus()
    }

    private func refreshInstallStatus() async {
        // Delegate to the pipeline so all observers (mic button, wizard,
        // future banners) stay coherent. The pipeline dedupes concurrent
        // calls and publishes to `installSnapshot` which this view reads.
        await self.pipeline.refreshDaemonState()
    }

    private func reinstall() async {
        self.isReinstalling = true
        defer { self.isReinstalling = false }
        do {
            try await DaemonClient.shared.startVoiceInstall()
        } catch {
            self.error = error.localizedDescription
            return
        }
        // Also kick the client-side STT (Parakeet or Qwen3 depending on the
        // selected language) so a "Reinstall all" actually touches both
        // sides. FluidAudio's loadModels is idempotent — already-present
        // models return fast; missing ones re-download.
        Task { @MainActor in
            await self.pipeline.ensureSttLoaded()
        }
        // Poll until terminal so the model rows update live. 5min ceiling
        // covers slow-network installs of the larger Qwen3 bundle;
        // daemon-side single-flight makes a second click safely no-op.
        self.statusPollTask?.cancel()
        self.statusPollTask = Task {
            for _ in 0..<600 where !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 500_000_000)
                await self.refreshInstallStatus()
                if let s = self.installStatus, s.isTerminal {
                    // Wake the overlay's MicButton (and any other observer)
                    // immediately rather than waiting for its 30s poll tick.
                    NotificationCenter.default.post(name: .bobeVoiceConfigChanged, object: nil)
                    return
                }
            }
        }
    }

    // MARK: - Helpers

    private func errorBanner(_ message: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(self.theme.colors.primary)
            Text(message)
                .font(.system(size: 12))
                .foregroundStyle(self.theme.colors.primary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 8).fill(self.theme.colors.primary.opacity(0.08)))
    }

    private func savedToast(_ message: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(self.theme.colors.secondary)
            Text(message)
                .font(.system(size: 11))
                .foregroundStyle(self.theme.colors.secondary)
            Spacer()
        }
        .transition(.opacity)
    }

    private func binding<V>(
        _ keyPath: WritableKeyPath<DaemonSettings, V>,
        fallback: @autoclosure @escaping () -> V
    ) -> Binding<V> {
        Binding(
            get: { self.settings?[keyPath: keyPath] ?? fallback() },
            set: { newValue in
                guard var current = self.settings else { return }
                current[keyPath: keyPath] = newValue
                self.settings = current
                self.debounceSave()
            }
        )
    }

    private func debounceSave() {
        self.debouncer.debounce { await self.persist() }
    }

    private func persist() async {
        guard let settings = self.settings else { return }
        var req = SettingsUpdateRequest()
        req.voiceEnabled = settings.voiceEnabled
        req.voicePersona = settings.voicePersona
        req.voiceSpeed = settings.voiceSpeed
        req.voiceSttLanguage = settings.voiceSttLanguage
        req.voicePauseSensitivity = settings.voicePauseSensitivity
        req.voiceShowPartialCaption = settings.voiceShowPartialCaption
        do {
            let resp = try await DaemonClient.shared.updateSettings(req)
            if resp.persistFailed == true {
                self.error = L10n.tr("settings.shared.action.persist_failed")
                self.savedMessage = nil
            } else {
                self.error = nil
                self.savedMessage = resp.message
                self.scheduleSavedToastDismiss()
                // Trigger a background load of the (now-active) engine. The
                // pipeline's `bobeVoiceConfigChanged` listener will refresh
                // its activeSttLanguage first; this kicks off the download
                // for the new engine if needed.
                Task { @MainActor in
                    await self.pipeline.refreshDaemonState()
                    await self.pipeline.ensureSttLoaded()
                }
                NotificationCenter.default.post(name: .bobeVoiceConfigChanged, object: nil)
            }
        } catch {
            self.error = error.localizedDescription
            self.savedMessage = nil
        }
    }

    private func scheduleSavedToastDismiss() {
        self.debouncer.scheduleToastClear { self.savedMessage = nil }
    }
}

// KokoroVoices + VoiceLanguages enums live in `VoiceSettingsEnums.swift`.

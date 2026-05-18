import SwiftUI

/// Voice settings — daemon-side defaults for the persona/speed used when the
/// Hello handshake doesn't override, plus the master enable toggle, plus the
/// voice-model install status & reinstall affordance.
///
/// All fields hot-swap (no daemon restart). Voice toggle takes effect at the
/// next `/voice/stream` connect. Persona/speed apply on the next turn.
struct VoicePanel: View {
    @State private var pipeline = VoicePipeline.shared
    @State private var store = SettingsStore.shared
    @State private var isReinstalling = false
    /// Install-side errors live separate from `store.error` (which is the
    /// settings persist channel). Reinstall failures don't belong in the
    /// settings-save banner — they're a transient daemon-RPC concern.
    @State private var installError: String?
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
                    .bobeTextStyle(.windowTitle)
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("settings.voice.description"))
                    .bobeTextStyle(.settingsBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)

                if let error = self.store.error ?? self.installError {
                    SettingsErrorBanner(message: error)
                }
                if let savedMessage = self.store.savedMessage {
                    SettingsSavedToast(message: savedMessage)
                }

                if self.store.settings != nil {
                    self.engineSection
                    self.modelsSection
                } else if self.store.isLoading {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.engine.loading"))
                            .bobeTextStyle(.settingsBody)
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
                            .bobeTextStyle(.settingsBody)
                            .foregroundStyle(self.theme.colors.textMuted)
                            .fixedSize(horizontal: false, vertical: true)
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
            self.store.cancelToast()
        }
        .onChange(of: self.pipeline.sttStatus) { _, newValue in
            self.startSttProgressTickerIfNeeded(for: newValue)
        }
    }

    // MARK: - Sections

    private var engineSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.voice.section.preferences"),
            icon: "waveform",
            description: L10n.tr("settings.voice.description")
        ) {
            VStack(alignment: .leading, spacing: 12) {
                SettingsRow(
                    label: L10n.tr("settings.voice.enabled"),
                    description: L10n.tr("settings.voice.enabled.description")
                ) {
                    BobeToggle(isOn: self.store.binding(\.voiceEnabled, fallback: true))
                }

                // Language picker — drives client-side engine selection.
                // English uses FluidAudio Parakeet EOU today; other languages
                // are visible but marked "coming soon" until Qwen3-ASR wiring
                // lands (per the language matrix in docs/voice-architecture.md).
                SettingsRow(
                    label: L10n.tr("settings.voice.language"),
                    description: L10n.tr("settings.voice.language.description")
                ) {
                    BobeMenuPicker(
                        selection: self.store.binding(\.voiceSttLanguage, fallback: "en"),
                        options: VoiceLanguages.all,
                        label: VoiceLanguages.displayName(for:),
                        width: 240
                    )
                }

                SettingsRow(
                    label: L10n.tr("settings.voice.pause_sensitivity"),
                    description: L10n.tr("settings.voice.pause_sensitivity.description")
                ) {
                    BobeMenuPicker(
                        selection: self.store.binding(\.voicePauseSensitivity, fallback: "balanced"),
                        options: ["tight", "balanced", "patient"],
                        label: { L10n.tr("settings.voice.pause_sensitivity.\($0)") },
                        width: 240
                    )
                }

                SettingsRow(
                    label: L10n.tr("settings.voice.persona"),
                    description: L10n.tr("settings.voice.persona.description")
                ) {
                    BobeMenuPicker(
                        selection: self.store.binding(\.voicePersona, fallback: "af_bella"),
                        options: KokoroVoices.allSlots,
                        label: KokoroVoices.displayName(for:),
                        width: 240
                    )
                }

                SettingsRow(
                    label: L10n.tr("settings.voice.speed"),
                    description: L10n.tr("settings.voice.speed.description")
                ) {
                    HStack(spacing: 12) {
                        Slider(
                            value: self.store.binding(\.voiceSpeed, fallback: 1.0),
                            in: 0.5 ... 2.0,
                            step: 0.05
                        )
                        .frame(width: 180)
                        .accessibilityLabel(L10n.tr("settings.voice.speed"))
                        .accessibilityHint(L10n.tr("settings.voice.speed.description"))
                        .accessibilityValue(String(format: "%.2f×", Double(self.store.settings?.voiceSpeed ?? 1.0)))
                        Text(String(format: "%.2f×", Double(self.store.settings?.voiceSpeed ?? 1.0)))
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(self.theme.colors.textMuted)
                            .frame(width: 56, alignment: .trailing)
                            .accessibilityHidden(true)
                    }
                }

                SettingsRow(
                    label: L10n.tr("settings.voice.partial_caption"),
                    description: L10n.tr("settings.voice.partial_caption.description")
                ) {
                    BobeToggle(isOn: self.store.binding(\.voiceShowPartialCaption, fallback: true))
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
                // Daemon-side TTS (Kokoro). Friendly name by default, codename in Expert.
                VoiceModelCard(
                    name: ExpertMode.shared.isEnabled
                        ? L10n.tr("settings.voice.model.tts.name_expert")
                        : L10n.tr("settings.voice.model.tts.name"),
                    purpose: L10n.tr("settings.voice.model.tts.purpose"),
                    sizeHint: L10n.tr("settings.voice.model.tts.size"),
                    location: "~/.bobe/models/kokoro-multi-lang-v1_0/",
                    status: self.kokoroStatus,
                    daemonProgress: self.installStatus?.models.first(where: { $0.kind == VoiceWire.modelKindTts }),
                    onInstall: { await self.installSingle(daemon: true) },
                    onUninstall: { await self.uninstall(path: "~/.bobe/models/kokoro-multi-lang-v1_0/") }
                )

                // Client-side STT — English (FluidAudio Parakeet EOU).
                VoiceModelCard(
                    name: ExpertMode.shared.isEnabled
                        ? L10n.tr("settings.voice.model.stt_en.name_expert")
                        : L10n.tr("settings.voice.model.stt_en.name"),
                    purpose: L10n.tr("settings.voice.model.stt_en.purpose"),
                    sizeHint: L10n.tr("settings.voice.model.stt_en.size"),
                    location: "~/Library/Application Support/FluidAudio/Models/parakeet-eou-streaming/",
                    status: self.parakeetStatus,
                    daemonProgress: nil,
                    onInstall: { await self.installSingle(daemon: false) },
                    onUninstall: { await self.uninstall(path: "~/Library/Application Support/FluidAudio/Models/parakeet-eou-streaming/") }
                )

                // Client-side STT — Mandarin (FluidAudio Qwen3-ASR + VAD).
                // Card is always visible so the user can see the install
                // weight before switching languages; only consumed when
                // `voice.stt_language` is non-English (Qwen3 covers them all).
                VoiceModelCard(
                    name: ExpertMode.shared.isEnabled
                        ? L10n.tr("settings.voice.model.stt_multi.name_expert")
                        : L10n.tr("settings.voice.model.stt_multi.name"),
                    purpose: L10n.tr("settings.voice.model.stt_multi.purpose"),
                    sizeHint: L10n.tr("settings.voice.model.stt_multi.size"),
                    location: "~/Library/Application Support/FluidAudio/Models/qwen3-asr-0.6b-coreml/",
                    status: self.qwen3Status,
                    daemonProgress: nil,
                    onInstall: { await self.installSingle(daemon: false) },
                    onUninstall: { await self.uninstall(path: "~/Library/Application Support/FluidAudio/Models/qwen3-asr-0.6b-coreml/") }
                )

                // Explain where models live — the FluidAudio library caches
                // under `~/Library/Application Support/FluidAudio/` by
                // convention (not configurable today); Kokoro lives under
                // `~/.bobe/` because we control that one. Surface this so
                // the user isn't surprised by the split layout.
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "info.circle")
                        .font(.system(size: 10))
                        .foregroundStyle(self.theme.colors.textMuted)
                    Text(L10n.tr("settings.voice.section.models.location_note"))
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 0)
                }
                .padding(.top, 4)

                // Global reinstall — restarts both pipelines in parallel.
                HStack(spacing: 8) {
                    Button(self.isReinstalling
                        ? L10n.tr("settings.voice.reinstalling")
                        : L10n.tr("settings.voice.reinstall")
                    ) {
                        Task { await self.reinstall() }
                    }
                    .bobeButton(.primary, size: .small)
                    .disabled(self.isReinstalling)
                    .accessibilityLabel(L10n.tr("settings.voice.reinstall.accessibility"))
                    Text(self.isReinstalling
                        ? L10n.tr("settings.voice.reinstall.running")
                        : L10n.tr("settings.voice.reinstall.description")
                    )
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)
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
        let activeLanguage = self.store.settings?.voiceSttLanguage ?? "en"
        if activeLanguage != "en" {
            return self.cardStatus(for: FluidAudioQwen3ModelPresence.self)
        }
        return FluidAudioQwen3ModelPresence.isInstalled() ? .installed : .missing
    }

    private func cardStatus<P: FluidAudioPresence>(for presence: P.Type) -> VoiceModelCard.Status {
        switch self.pipeline.sttStatus {
        case .ready: .installed
        case .downloading:
            .downloading(
                bytesDownloaded: P.observedBytes(),
                bytesTotal: P.approximateTotalBytes,
                percent: P.observedPercent()
            )
        case let .failed(msg): .failed(msg)
        case .notLoaded:
            P.isInstalled() ? .installed : .missing
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
                    try? await Task.sleep(for: .milliseconds(500))
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
        await self.store.loadIfNeeded()
        await self.pipeline.refreshDaemonState()
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
        self.installError = nil
        do {
            try await DaemonClient.shared.startVoiceInstall()
        } catch {
            self.installError = error.localizedDescription
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
            for _ in 0 ..< 600 where !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(500))
                await self.refreshInstallStatus()
                if self.installStatus?.isTerminal == true {
                    return
                }
            }
        }
    }

    /// Install a single model. Daemon path covers Kokoro (it batches every
    /// missing daemon-side model via the same endpoint, but invoking it
    /// when only Kokoro is missing only fetches Kokoro). Client path runs
    /// FluidAudio's loadModels, which downloads the active-language STT
    /// engine to the FluidAudio cache directory.
    private func installSingle(daemon: Bool) async {
        self.installError = nil
        do {
            if daemon {
                try await DaemonClient.shared.startVoiceInstall()
            } else {
                await self.pipeline.ensureSttLoaded()
            }
        } catch {
            self.installError = error.localizedDescription
        }
        await self.refreshInstallStatus()
    }

    /// Uninstall by deleting the model's on-disk directory. Daemon and
    /// FluidAudio both probe filesystem presence on every snapshot, so the
    /// status flips to `.missing` on the next refresh — no special
    /// endpoint needed. Idempotent: nonexistent path is a successful
    /// no-op.
    private func uninstall(path: String) async {
        let expanded = NSString(string: path).expandingTildeInPath
        let url = URL(fileURLWithPath: expanded, isDirectory: true)
        if FileManager.default.fileExists(atPath: url.path) {
            try? FileManager.default.removeItem(at: url)
        }
        await self.refreshInstallStatus()
    }
}

// KokoroVoices + VoiceLanguages enums live in `VoiceSettingsEnums.swift`.

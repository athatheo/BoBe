import SwiftUI

/// Voice settings — daemon-side defaults for the persona/speed used when the
/// Hello handshake doesn't override, plus the master enable toggle, plus the
/// voice-model install status & reinstall affordance.
///
/// All fields hot-swap (no daemon restart). Voice toggle takes effect at the
/// next `/voice/stream` connect. Persona/speed apply on the next turn.
struct VoicePanel: View {
    @Environment(VoicePipeline.self) private var pipeline
    @Environment(SettingsStore.self) private var store
    @Environment(ExpertMode.self) private var expertMode
    @State private var ttsPreference = VoiceTtsPreference.shared
    @State private var isReinstalling = false
    /// Install-side errors live separate from `store.error` (which is the
    /// settings persist channel). Reinstall failures don't belong in the
    /// settings-save banner — they're a transient daemon-RPC concern.
    @State private var installError: String?
    @State private var clientTtsInstalled = FluidAudioSupertonicModelPresence.isInstalled()
    @State private var statusPollTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

    /// Install snapshot — pulled from the central `VoicePipeline` observable
    /// so the wizard, mic button, and settings card all see the same data.
    private var installStatus: VoiceInstallSnapshot? {
        self.pipeline.installSnapshot
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
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
            // Let any in-flight persist drain in the background — closing
            // Settings mid-edit must not silently drop the last keystroke.
            self.store.cancelToast()
        }
    }

    // MARK: - Sections

    private var engineSection: some View {
        CollapsibleSection(
            title: L10n.tr("settings.voice.section.preferences"),
            icon: "waveform",
            description: nil
        ) {
            VStack(alignment: .leading, spacing: 12) {
                SettingsRow(
                    label: L10n.tr("settings.voice.enabled"),
                    description: L10n.tr("settings.voice.enabled.description")
                ) {
                    BobeToggle(
                        isOn: self.store.binding(\.voiceEnabled, fallback: true),
                        accessibilityLabel: L10n.tr("settings.voice.enabled")
                    )
                }

                // English uses Parakeet EOU; every other listed language uses
                // Nemotron's full multilingual streaming model.
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

                if self.expertMode.isEnabled {
                    SettingsRow(
                        label: L10n.tr("settings.voice.tts_backend"),
                        description: L10n.tr("settings.voice.tts_backend.description")
                    ) {
                        BobeMenuPicker(
                            selection: Binding(
                                get: { self.ttsPreference.backend.rawValue },
                                set: { raw in
                                    guard let backend = VoiceTtsBackend(rawValue: raw) else { return }
                                    self.ttsPreference.setBackend(backend)
                                }
                            ),
                            options: VoiceTtsBackend.allCases.map(\.rawValue),
                            label: {
                                VoiceTtsBackend(rawValue: $0)?.label ?? $0
                            },
                            width: 240
                        )
                    }
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
                    BobeToggle(
                        isOn: self.store.binding(\.voiceShowPartialCaption, fallback: true),
                        accessibilityLabel: L10n.tr("settings.voice.partial_caption")
                    )
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
                if self.expertMode.isEnabled,
                   self.ttsPreference.backend == .clientSupertonic {
                    VoiceModelCard(
                        name: L10n.tr("settings.voice.model.supertonic.name"),
                        purpose: L10n.tr("settings.voice.model.supertonic.purpose"),
                        sizeHint: L10n.tr("settings.voice.model.supertonic.size"),
                        location: "~/.cache/fluidaudio/Models/supertonic-3/",
                        status: self.clientTtsInstalled ? .installed : .missing,
                        daemonProgress: nil,
                        onInstall: { await self.prepareClientTts() },
                        onUninstall: nil
                    )
                }

                // Daemon-side TTS (Kokoro). Friendly name by default, codename in Expert.
                VoiceModelCard(
                    name: self.expertMode.isEnabled
                        ? L10n.tr("settings.voice.model.tts.name_expert")
                        : L10n.tr("settings.voice.model.tts.name"),
                    purpose: L10n.tr("settings.voice.model.tts.purpose"),
                    sizeHint: L10n.tr("settings.voice.model.tts.size"),
                    location: self.expertMode.isEnabled
                        ? "~/.bobe/models/kokoro-multi-lang-v1_0/"
                        : nil,
                    status: self.kokoroStatus,
                    daemonProgress: self.installStatus?.models.first(where: { $0.kind == VoiceWire.modelKindTts }),
                    onInstall: { await self.installSingle(daemon: true) },
                    onUninstall: { await self.uninstall(path: "~/.bobe/models/kokoro-multi-lang-v1_0/") }
                )

                // Client-side STT — English (FluidAudio Parakeet EOU).
                VoiceModelCard(
                    name: self.expertMode.isEnabled
                        ? L10n.tr("settings.voice.model.stt_en.name_expert")
                        : L10n.tr("settings.voice.model.stt_en.name"),
                    purpose: L10n.tr("settings.voice.model.stt_en.purpose"),
                    sizeHint: L10n.tr("settings.voice.model.stt_en.size"),
                    location: self.expertMode.isEnabled
                        ? "~/Library/Application Support/FluidAudio/Models/parakeet-eou-streaming/"
                        : nil,
                    status: self.parakeetStatus,
                    daemonProgress: nil,
                    onInstall: { await self.installSingle(daemon: false, language: "en") },
                    onUninstall: {
                        await self.uninstall(
                            path: "~/Library/Application Support/FluidAudio/Models/parakeet-eou-streaming/",
                            sttLanguage: "en"
                        )
                    }
                )

                // Client-side STT — multilingual Nemotron + Silero VAD.
                VoiceModelCard(
                    name: self.expertMode.isEnabled
                        ? L10n.tr("settings.voice.model.stt_multi.name_expert")
                        : L10n.tr("settings.voice.model.stt_multi.name"),
                    purpose: L10n.tr("settings.voice.model.stt_multi.purpose"),
                    sizeHint: L10n.tr("settings.voice.model.stt_multi.size"),
                    location: self.expertMode.isEnabled
                        ? "~/Library/Application Support/FluidAudio/Models/nemotron-multilingual/"
                        : nil,
                    status: self.nemotronStatus,
                    daemonProgress: nil,
                    onInstall: {
                        let language = self.store.settings?.voiceSttLanguage ?? "zh"
                        await self.installSingle(
                            daemon: false,
                            language: language == "en" ? "zh" : language
                        )
                    },
                    onUninstall: {
                        await self.uninstall(
                            path: "~/Library/Application Support/FluidAudio/Models/nemotron-multilingual/",
                            sttLanguage: "zh"
                        )
                    }
                )

                // Explain where models live — the FluidAudio library caches
                // under `~/Library/Application Support/FluidAudio/` by
                // convention (not configurable today); Kokoro lives under
                // `~/.bobe/` because we control that one. Surface this so
                // the user isn't surprised by the split layout.
                if self.expertMode.isEnabled {
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
                }

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
            return .failed(self.installStatus?.error ?? progress.status)
        }
        return .missing
    }

    private var parakeetStatus: VoiceModelCard.Status {
        self.cardStatus(for: FluidAudioModelPresence.self, language: "en")
    }

    private var nemotronStatus: VoiceModelCard.Status {
        self.cardStatus(for: FluidAudioNemotronModelPresence.self, language: "zh")
    }

    private func cardStatus<P: FluidAudioPresence>(
        for presence: P.Type,
        language: String
    ) -> VoiceModelCard.Status {
        let wantsEnglish = language == "en"
        let activeLanguage = self.store.settings?.voiceSttLanguage ?? "en"
        let activeMatches = (activeLanguage == "en") == wantsEnglish
        let downloadMatches = self.pipeline.sttDownloadLanguage.map {
            ($0 == "en") == wantsEnglish
        } ?? false

        if downloadMatches {
            return .downloading(
                bytesDownloaded: 0,
                bytesTotal: nil,
                percent: self.pipeline.sttLoadProgress?.percent
            )
        }
        if activeMatches {
            switch self.pipeline.sttStatus {
            case .ready: return .installed
            case let .failed(message): return .failed(message)
            case .downloading:
                return .downloading(bytesDownloaded: 0, bytesTotal: nil, percent: nil)
            case .notLoaded: break
            }
        }
        return P.isInstalled() ? .installed : .missing
    }

    private var modelsSummary: String {
        switch self.pipeline.readiness {
        case .ready:
            return L10n.tr("settings.voice.installed_label")
        case .installing, .modelsMissing, .failed:
            // Partial when one side is on disk; missing otherwise. Surface
            // through underlying signals so the label is honest rather than
            // collapsing to a single "in progress" word.
            let selectedTtsReady = switch self.ttsPreference.backend {
            case .serverKokoro: self.installStatus?.installed.tts ?? false
            case .clientSupertonic: self.clientTtsInstalled
            }
            let sttReady = self.pipeline.sttStatus == .ready
            if selectedTtsReady || sttReady {
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
        self.clientTtsInstalled = FluidAudioSupertonicModelPresence.isInstalled()
    }

    private func prepareClientTts() async {
        do {
            try await self.pipeline.clientTtsEngine.prepare()
            self.clientTtsInstalled = true
            self.installError = nil
        } catch {
            self.installError = error.localizedDescription
        }
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
            try await self.pipeline.prepareSelectedTts()
        } catch {
            self.installError = error.localizedDescription
            return
        }
        // Also kick the client-side STT (Parakeet or Nemotron depending on the
        // selected language) so a "Reinstall all" actually touches both
        // sides. FluidAudio's loadModels is idempotent — already-present
        // models return fast; missing ones re-download.
        Task { @MainActor in
            await self.pipeline.ensureSttLoaded()
        }
        // Poll until terminal so the model rows update live. 5min ceiling
        // covers slow-network installs of the larger multilingual bundle;
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
    private func installSingle(daemon: Bool, language: String? = nil) async {
        self.installError = nil
        do {
            if daemon {
                try await DaemonClient.shared.startVoiceInstall()
            } else {
                await self.pipeline.ensureSttLoaded(for: language)
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
    private func uninstall(path: String, sttLanguage: String? = nil) async {
        if let sttLanguage {
            await self.pipeline.unloadSttModel(for: sttLanguage)
        }
        let expanded = NSString(string: path).expandingTildeInPath
        let url = URL(fileURLWithPath: expanded, isDirectory: true)
        if FileManager.default.fileExists(atPath: url.path) {
            try? FileManager.default.removeItem(at: url)
        }
        await self.refreshInstallStatus()
    }
}

// KokoroVoices + VoiceLanguages enums live in `VoiceSettingsEnums.swift`.

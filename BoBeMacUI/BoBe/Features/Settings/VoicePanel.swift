import SwiftUI

/// Voice settings — daemon-side defaults for the persona/speed used when the
/// Hello handshake doesn't override, plus the master enable toggle, plus the
/// voice-model install status & reinstall affordance.
///
/// All fields hot-swap (no daemon restart). Voice toggle takes effect at the
/// next `/voice/stream` connect. Persona/speed apply on the next turn.
struct VoicePanel: View {
    @State private var settings: DaemonSettings?
    @State private var installStatus: VoiceInstallSnapshot?
    @State private var isLoading = false
    @State private var isReinstalling = false
    @State private var error: String?
    @State private var savedMessage: String?
    @State private var saveTask: Task<Void, Never>?
    @State private var savedToastTask: Task<Void, Never>?
    @State private var statusPollTask: Task<Void, Never>?
    @Environment(\.theme) private var theme

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
                }
            }
            .padding(24)
        }
        .task {
            await self.loadAll()
        }
        .onDisappear {
            self.statusPollTask?.cancel()
            // Don't cancel saveTask — let any pending PATCH complete in the
            // background even after the panel closes, otherwise rapid edits
            // followed by closing Settings would silently lose changes.
            self.savedToastTask?.cancel()
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
                        Text(String(format: "%.2f×", Double(self.settings?.voiceSpeed ?? 1.0)))
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(self.theme.colors.textMuted)
                            .frame(width: 56, alignment: .trailing)
                    }
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
            VStack(alignment: .leading, spacing: 10) {
                if let models = self.installStatus?.models {
                    ForEach(models, id: \.id) { model in
                        VoiceModelRow(model: model)
                    }
                }
                Button(self.isReinstalling
                    ? L10n.tr("settings.voice.reinstalling")
                    : L10n.tr("settings.voice.reinstall")
                ) {
                    Task { await self.reinstall() }
                }
                .bobeButton(.primary, size: .small)
                .disabled(self.isReinstalling)
                .padding(.top, 4)
            }
        }
    }

    private var modelsSummary: String {
        guard let installed = self.installStatus?.installed else { return "" }
        if installed.allPresent { return L10n.tr("settings.voice.installed_label") }
        let any = installed.streamingStt || installed.tts || installed.vad || installed.smartTurn
        return any
            ? L10n.tr("settings.voice.partial_label")
            : L10n.tr("settings.voice.missing_label")
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
        do {
            self.installStatus = try await DaemonClient.shared.voiceInstallStatus()
        } catch {
            // Status endpoint failure is non-fatal — leave the section blank
            // rather than show a top-level error banner.
            self.installStatus = nil
        }
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
        // Poll until terminal so the model rows update live. 5min ceiling
        // covers slow-network installs of the 600MB bundle; daemon-side
        // single-flight makes a second click safely no-op.
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
        self.saveTask?.cancel()
        self.saveTask = Task {
            try? await Task.sleep(for: .seconds(0.6))
            guard !Task.isCancelled else { return }
            await self.persist()
        }
    }

    private func persist() async {
        guard let settings = self.settings else { return }
        var req = SettingsUpdateRequest()
        req.voiceEnabled = settings.voiceEnabled
        req.voicePersona = settings.voicePersona
        req.voiceSpeed = settings.voiceSpeed
        do {
            let resp = try await DaemonClient.shared.updateSettings(req)
            if resp.persistFailed == true {
                self.error = L10n.tr("settings.shared.action.persist_failed")
                self.savedMessage = nil
            } else {
                self.error = nil
                self.savedMessage = resp.message
                self.scheduleSavedToastDismiss()
                NotificationCenter.default.post(name: .bobeVoiceConfigChanged, object: nil)
            }
        } catch {
            self.error = error.localizedDescription
            self.savedMessage = nil
        }
    }

    private func scheduleSavedToastDismiss() {
        self.savedToastTask?.cancel()
        self.savedToastTask = Task {
            try? await Task.sleep(for: .seconds(2.4))
            guard !Task.isCancelled else { return }
            self.savedMessage = nil
        }
    }
}

/// Kokoro v1.0 multilingual voice slot table. Mirrored from the daemon's
/// `BoBeService/src/speech/local_kokoro.rs::voice_id` enum.
///
/// **Split-brain warning**: there is no shared schema. Both lists must update
/// together when Kokoro publishes a new voice. The Rust side is authoritative
/// (the daemon rejects unknown ids); this Swift list controls only the UI
/// picker. The `voice_persona_round_trip` integration test (when it lands)
/// will assert that every entry here is accepted by the daemon.
private enum KokoroVoices {
    static let allSlots: [String] = [
        "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica",
        "af_kore", "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky",
        "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam",
        "am_michael", "am_onyx", "am_puck", "am_santa",
        "bf_alice", "bf_emma", "bf_isabella", "bf_lily",
        "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
        "ef_dora", "em_alex",
        "ff_siwis",
        "hf_alpha", "hf_beta", "hm_omega", "hm_psi",
        "if_sara", "im_nicola",
        "jf_alpha", "jf_gongitsune", "jf_nezumi", "jf_tebukuro", "jm_kumo",
        "pf_dora", "pm_alex", "pm_santa",
        "zf_xiaobei", "zf_xiaoni", "zf_xiaoxiao", "zf_xiaoyi",
        "zm_yunjian", "zm_yunxi", "zm_yunxia", "zm_yunyang",
    ]

    /// Friendly display name for a slot (e.g. `af_bella` → "Bella (English ♀)").
    static func displayName(for slot: String) -> String {
        let parts = slot.split(separator: "_", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return slot }
        let prefix = parts[0]
        let name = parts[1].capitalized
        let lang = self.languageLabel(forPrefix: prefix)
        return "\(name) (\(lang))"
    }

    private static func languageLabel(forPrefix prefix: String) -> String {
        let lang: String
        switch prefix.first {
        case "a": lang = "English"
        case "b": lang = "British English"
        case "e": lang = "Spanish"
        case "f": lang = "French"
        case "h": lang = "Hindi"
        case "i": lang = "Italian"
        case "j": lang = "Japanese"
        case "p": lang = "Portuguese"
        case "z": lang = "Mandarin"
        default: lang = "?"
        }
        let gender: String
        switch prefix.last {
        case "f": gender = "♀"
        case "m": gender = "♂"
        default: gender = ""
        }
        return gender.isEmpty ? lang : "\(lang) \(gender)"
    }
}

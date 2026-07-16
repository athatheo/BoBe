import SwiftUI

// MARK: - Voice setup step

struct VoiceSetupStepView: View {
    let onContinue: () -> Void

    @Environment(\.theme) private var theme
    @State private var pipeline = VoicePipeline.shared
    @State private var phase: Phase = .checking
    @State private var errorMessage: String?

    enum Phase {
        case checking, idle, installing, complete, failed, skipped
    }

    /// Reads through the pipeline's shared snapshot so the wizard, mic
    /// button, and settings card all see the same daemon install state.
    private var status: VoiceInstallSnapshot? {
        self.pipeline.installSnapshot
    }

    /// True once daemon-side install completes AND FluidAudio is loaded.
    /// Pulled straight from `pipeline.readiness` for consistency with every
    /// other consumer; falls back through the underlying signals only as a
    /// readability convenience in the per-model UI.
    private var allReady: Bool {
        self.pipeline.readiness == .ready
    }

    private var selectedTtsBackend: VoiceTtsBackend {
        VoiceTtsPreference.shared.backend
    }

    private var visibleDaemonModels: [VoiceModelProgress] {
        self.selectedTtsBackend == .serverKokoro ? (self.status?.models ?? []) : []
    }

    private var sttLabel: String {
        switch self.pipeline.sttStatus {
        case .notLoaded: return L10n.tr("setup.voice.stt_label.waiting")
        case .downloading:
            let percent = self.pipeline.sttLoadProgress?.percent ?? 0
            return String(
                format: L10n.tr("setup.voice.stt_label.progress_format"),
                percent
            )
        case .ready: return L10n.tr("setup.voice.stt_label.ready")
        case let .failed(msg): return String(format: L10n.tr("setup.voice.stt_label.failed_format"), msg)
        }
    }

    private var sttModel: VoiceModelProgress {
        let status = switch self.pipeline.sttStatus {
        case .ready: "already installed"
        case .downloading: "downloading"
        case .failed: "failed"
        case .notLoaded: "pending"
        }
        return VoiceModelProgress(
            kind: "stt",
            label: L10n.tr("setup.voice.model.stt"),
            status: status,
            bytesDownloaded: 0,
            bytesTotal: nil,
            percent: self.pipeline.sttLoadProgress?.percent
        )
    }

    var body: some View {
        VStack(spacing: 18) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.voice.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.voice.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }

            self.bodyContent

            Spacer()

            self.actionRow
        }
        .task { await self.refreshStatus() }
    }

    @ViewBuilder
    private var bodyContent: some View {
        switch self.phase {
        case .checking:
            ProgressView()
                .progressViewStyle(.circular)
                .padding(.top, 20)
        case .idle:
            self.installPrompt
        case .installing:
            self.progressList
        case .complete:
            self.completeSummary
        case .failed:
            self.failureSummary
        case .skipped:
            Text(L10n.tr("setup.voice.skipped_note"))
                .bobeTextStyle(.setupBody)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
        }
    }

    private var installPrompt: some View {
        VStack(spacing: 12) {
            ForEach(self.visibleDaemonModels, id: \.id) { model in
                VoiceModelRow(model: model)
            }
            VoiceModelRow(model: self.sttModel)
        }
        .padding(.vertical, 8)
    }

    private var progressList: some View {
        VStack(spacing: 12) {
            ForEach(self.visibleDaemonModels, id: \.id) { model in
                VoiceModelRow(model: model)
            }
            HStack(spacing: 10) {
                switch self.pipeline.sttStatus {
                case .ready:
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(self.theme.colors.secondary)
                case .downloading, .notLoaded:
                    ProgressView()
                        .progressViewStyle(.circular)
                        .controlSize(.small)
                case .failed:
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(self.theme.colors.primary)
                }
                Text(self.sttLabel)
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                Spacer()
            }
            .padding(.horizontal, 4)
        }
        .task { await self.poll() }
    }

    private var completeSummary: some View {
        HStack(spacing: 10) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(self.theme.colors.secondary)
            Text(L10n.tr("setup.voice.complete"))
                .bobeTextStyle(.setupBody)
                .foregroundStyle(self.theme.colors.text)
        }
        .padding(.vertical, 16)
    }

    private var failureSummary: some View {
        VStack(spacing: 8) {
            HStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(self.theme.colors.primary)
                Text(L10n.tr("setup.voice.failed"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.text)
            }
            if let errorMessage = self.errorMessage {
                Text(errorMessage)
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        switch self.phase {
        case .checking:
            EmptyView()
        case .idle:
            HStack(spacing: 12) {
                Button(L10n.tr("setup.voice.skip")) { self.phase = .skipped }
                    .bobeButton(.secondary, size: .regular)
                Button(L10n.tr("setup.voice.install_button")) { Task { await self.kickoff() } }
                    .bobeButton(.primary, size: .regular)
                    .keyboardShortcut(.defaultAction)
            }
        case .installing:
            Button(L10n.tr("setup.voice.cancel")) {
                Task { await self.cancelAll() }
            }
            .bobeButton(.secondary, size: .regular)
        case .complete, .failed, .skipped:
            Button(L10n.tr("setup.voice.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        }
    }

    private func refreshStatus() async {
        await self.pipeline.refreshDaemonState()
        guard let s = self.status else {
            self.errorMessage = L10n.tr("setup.voice.daemon_unreachable")
            self.phase = .failed
            return
        }
        // Reflect current FluidAudio state too; if either side is
        // unfinished we go back to idle/installing.
        if self.allReady {
            self.phase = .complete
        } else if s.isRunning || self.pipeline.sttStatus == .downloading {
            self.phase = .installing
            Task { await self.pipeline.ensureSttLoaded() }
        } else {
            self.phase = .idle
        }
    }

    private func kickoff() async {
        // Install the selected TTS backend and active client STT in parallel.
        Task { await self.pipeline.ensureSttLoaded() }
        do {
            try await self.pipeline.prepareSelectedTts()
            self.phase = .installing
        } catch {
            self.errorMessage = error.localizedDescription
            self.phase = .failed
        }
    }

    /// Cancel both download pipelines. FluidAudio 0.15's ModelHub preserves
    /// valid cached files and resumable partial state.
    private func cancelAll() async {
        async let daemon: Void = {
            try? await DaemonClient.shared.cancelVoiceInstall()
        }()
        async let stt: Void = self.pipeline.cancelSttLoading()
        _ = await (daemon, stt)
    }

    private func poll() async {
        while !Task.isCancelled, self.phase == .installing {
            try? await Task.sleep(for: .milliseconds(500))
            // STT failure short-circuits the poll loop AND cancels the
            // daemon-side install so the user doesn't see a half-running
            // background download after the failure screen.
            if case let .failed(msg) = self.pipeline.sttStatus {
                self.errorMessage = msg
                await self.cancelAll()
                self.phase = .failed
                return
            }
            await self.pipeline.refreshDaemonState()
            guard let s = self.status else {
                self.errorMessage = L10n.tr("setup.voice.daemon_unreachable")
                self.phase = .failed
                return
            }
            if self.allReady {
                self.phase = .complete
                return
            }
            switch s.status {
            case .failed:
                self.errorMessage = s.error ?? L10n.tr("setup.voice.poll_failure")
                self.phase = .failed; return
            case .canceled:
                self.phase = .idle; return
            case .idle, .running, .complete:
                // Daemon may already be complete while FluidAudio is
                // still downloading; keep polling until both finish.
                continue
            }
        }
    }
}

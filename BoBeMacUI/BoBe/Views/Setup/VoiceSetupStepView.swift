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

    private var sttLabel: String {
        switch pipeline.sttStatus {
        case .notLoaded: return "Speech recognition: waiting…"
        case .downloading:
            // FluidAudio doesn't expose progress, but we observe directory
            // size vs the known target to give a real percent. Target +
            // observer pair depend on which engine is loading (Parakeet
            // ~600 MB for English, Qwen3 + VAD ~1.75 GB for everything else).
            let isEnglish = self.pipeline.activeSttLanguage == "en"
            let percent = isEnglish
                ? FluidAudioModelPresence.observedPercent()
                : FluidAudioQwen3ModelPresence.observedPercent()
            let bytes = isEnglish
                ? FluidAudioModelPresence.observedBytes()
                : FluidAudioQwen3ModelPresence.observedBytes()
            let mb = Int(bytes / 1_048_576)
            let targetLabel = isEnglish ? "~600 MB" : "~1.75 GB"
            return "Speech recognition: downloading \(mb) MB (\(percent)% of \(targetLabel))…"
        case .ready: return "Speech recognition: ready"
        case .failed(let msg): return "Speech recognition: failed — \(msg)"
        }
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
            ForEach(self.status?.models ?? [], id: \.id) { model in
                VoiceModelRow(model: model)
            }
        }
        .padding(.vertical, 8)
    }

    private var progressList: some View {
        VStack(spacing: 12) {
            ForEach(self.status?.models ?? [], id: \.id) { model in
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
            self.errorMessage = "Daemon not reachable"
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
        // Kick off both installs in parallel — daemon-side Kokoro download
        // via /voice/install, client-side FluidAudio model via the Swift
        // pipeline. The view-attached `.task` on `progressList` then drives
        // `poll()` until both finish; that task gets free cancellation when
        // the view disappears (e.g., user closes the wizard mid-install).
        Task { await self.pipeline.ensureSttLoaded() }
        do {
            try await DaemonClient.shared.startVoiceInstall()
            self.phase = .installing
        } catch {
            self.errorMessage = error.localizedDescription
            self.phase = .failed
        }
    }

    /// Cancel-everything: stop the daemon install AND mark FluidAudio as
    /// failed (we can't truly cancel the FluidAudio download — see TODO).
    /// Used both for the explicit Cancel button and the STT-failed
    /// short-circuit so the user never sees a partially-running install.
    private func cancelAll() async {
        try? await DaemonClient.shared.cancelVoiceInstall()
        // TODO: also cancel FluidAudio load — requires FluidAudio
        // cancellation support (Task.cancel() may not interrupt the
        // HuggingFace download). For now the in-flight load continues
        // in the background and updates `sttStatus` when it finishes.
    }

    private func poll() async {
        while !Task.isCancelled, self.phase == .installing {
            try? await Task.sleep(nanoseconds: 500_000_000)
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
                self.errorMessage = "Daemon not reachable"
                self.phase = .failed
                return
            }
            if self.allReady {
                self.phase = .complete
                return
            }
            switch s.status {
            case .failed:
                self.errorMessage = L10n.tr("setup.voice.poll_failure")
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

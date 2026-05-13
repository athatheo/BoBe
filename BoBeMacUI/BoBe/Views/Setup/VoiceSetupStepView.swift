import SwiftUI

// MARK: - Voice setup step

struct VoiceSetupStepView: View {
    let onContinue: () -> Void

    @Environment(\.theme) private var theme
    @State private var status: VoiceInstallSnapshot?
    @State private var phase: Phase = .checking
    @State private var errorMessage: String?

    enum Phase {
        case checking, idle, installing, complete, failed, skipped
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
                Task { try? await DaemonClient.shared.cancelVoiceInstall() }
            }
            .bobeButton(.secondary, size: .regular)
        case .complete, .failed, .skipped:
            Button(L10n.tr("setup.voice.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        }
    }

    private func refreshStatus() async {
        do {
            let s = try await DaemonClient.shared.voiceInstallStatus()
            self.status = s
            if s.installed.allPresent {
                self.phase = .complete
            } else if s.isRunning {
                self.phase = .installing
            } else {
                self.phase = .idle
            }
        } catch {
            self.errorMessage = error.localizedDescription
            self.phase = .failed
        }
    }

    private func kickoff() async {
        do {
            try await DaemonClient.shared.startVoiceInstall()
            self.phase = .installing
            await self.poll()
        } catch {
            self.errorMessage = error.localizedDescription
            self.phase = .failed
        }
    }

    private func poll() async {
        while !Task.isCancelled, self.phase == .installing {
            try? await Task.sleep(nanoseconds: 500_000_000)
            do {
                let s = try await DaemonClient.shared.voiceInstallStatus()
                self.status = s
                if s.installed.allPresent {
                    self.phase = .complete
                    return
                }
                switch s.status {
                case "complete":
                    self.phase = .complete; return
                case "failed":
                    self.errorMessage = L10n.tr("setup.voice.poll_failure")
                    self.phase = .failed; return
                case "canceled":
                    self.phase = .idle; return
                default:
                    continue
                }
            } catch {
                self.errorMessage = error.localizedDescription
                self.phase = .failed
                return
            }
        }
    }
}

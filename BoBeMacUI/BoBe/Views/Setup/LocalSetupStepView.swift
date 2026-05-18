import SwiftUI

private enum LocalDefaults {
    static let chatModel = "qwen2.5:7b-instruct"
    static let batchModel = "qwen2.5:7b-instruct"
    static let visionModel = "qwen2.5vl:7b"
}

struct LocalSetupStepView: View {
    let onContinue: () -> Void

    @State private var snapshot: LocalRuntimeSnapshot?
    @State private var ramGB: Int? = systemMemoryGB()
    @State private var streamTask: Task<Void, Never>?
    @State private var hasStarted = false
    @State private var startError: String?
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.tr("setup.local.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.local.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
            }

            self.hardwareCard

            VStack(spacing: 12) {
                self.progressRow(
                    title: L10n.tr("setup.local.runtime"),
                    progress: self.snapshot?.runtime,
                    status: nil
                )
                self.modelRow(
                    title: L10n.tr("setup.local.chat_model"),
                    pull: self.snapshot?.chatModel
                )
                self.modelRow(
                    title: L10n.tr("setup.local.vision_model"),
                    pull: self.snapshot?.visionModel
                )
            }

            if let error = self.startError {
                Text(error)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.primary)
            }

            Spacer()

            self.actionRow
        }
        .task {
            await self.startInstall()
        }
        .onDisappear {
            self.streamTask?.cancel()
        }
    }

    @ViewBuilder
    private var hardwareCard: some View {
        if let ramGB = self.ramGB, ramGB < 16 {
            HStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.local.ram_warning"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .padding(10)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.theme.colors.tertiary.opacity(0.1))
            )
        }
    }

    private func progressRow(
        title: String,
        progress: LocalRuntimeDownload?,
        status: String?
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .bobeTextStyle(.setupHeading)
                Spacer()
                Text(progress.map { "\($0.percent ?? 0)%" } ?? "—")
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            BobeLinearProgressBar(progress: Double(progress?.percent ?? 0) / 100.0, tint: self.theme.colors.secondary)
            if let status {
                Text(status)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
    }

    private func modelRow(
        title: String,
        pull: LocalRuntimePull?
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .bobeTextStyle(.setupHeading)
                Spacer()
                Text(pull.map { "\($0.percent ?? 0)%" } ?? "—")
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            BobeLinearProgressBar(progress: Double(pull?.percent ?? 0) / 100.0, tint: self.theme.colors.secondary)
            if let status = pull?.status, !status.isEmpty {
                Text(status)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        let status = self.snapshot?.status ?? .running
        switch status {
        case .complete:
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case .canceled:
            Button(L10n.tr("setup.local.retry")) {
                Task { await self.startInstall() }
            }
            .bobeButton(.primary, size: .regular)
        case .failed:
            VStack(spacing: 8) {
                if let error = self.snapshot?.error {
                    Text(error)
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.primary)
                        .multilineTextAlignment(.center)
                }
                Button(L10n.tr("setup.local.retry")) {
                    Task { await self.startInstall() }
                }
                .bobeButton(.primary, size: .regular)
            }
        case .idle, .running:
            Button(L10n.tr("setup.local.cancel"), action: self.cancelInstall)
                .bobeButton(.secondary, size: .small)
        }
    }

    private func startInstall() async {
        self.startError = nil
        let request = LocalRuntimeInstallRequest(
            chatModel: LocalDefaults.chatModel,
            batchModel: LocalDefaults.batchModel,
            visionModel: LocalDefaults.visionModel
        )
        do {
            _ = try await DaemonClient.shared.startLocalRuntimeInstall(request)
            self.hasStarted = true
        } catch {
            // 409 = install already in flight; still subscribe to status.
            if !error.localizedDescription.lowercased().contains("conflict") {
                self.startError = error.localizedDescription
            }
        }
        self.streamTask?.cancel()
        self.streamTask = Task { @MainActor in
            do {
                try await DaemonClient.shared.streamLocalRuntimeStatus { snap in
                    Task { @MainActor in
                        self.snapshot = snap
                    }
                }
            } catch {
                self.startError = error.localizedDescription
            }
        }
    }

    private func cancelInstall() {
        Task {
            try? await DaemonClient.shared.cancelLocalRuntimeInstall()
        }
    }
}

private func systemMemoryGB() -> Int? {
    var memory: UInt64 = 0
    var size = MemoryLayout<UInt64>.size
    let result = sysctlbyname("hw.memsize", &memory, &size, nil, 0)
    guard result == 0 else { return nil }
    return Int(memory / (1024 * 1024 * 1024))
}

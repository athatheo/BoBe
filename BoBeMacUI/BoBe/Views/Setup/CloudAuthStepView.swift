import SwiftUI

private enum CloudAuthState {
    case checking
    case authenticated(login: String?, authType: String?)
    case unauthenticated(message: String?, cliPath: String?)
    case error(message: String)
}

struct CloudAuthStepView: View {
    let onContinue: () -> Void

    @State private var state: CloudAuthState = .checking
    @State private var pollTask: Task<Void, Never>?
    @State private var showingSignInSheet = false
    @State private var expertMode = ExpertMode.shared
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 20) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.cloud_auth.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.cloud_auth.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .padding(.top, 12)

            Spacer()

            self.statusCard

            Spacer()

            self.actionRow
        }
        .task { await self.refresh() }
        .onDisappear { self.pollTask?.cancel() }
        .sheet(isPresented: self.$showingSignInSheet) {
            CopilotSignInSheet(
                onSuccess: {
                    self.showingSignInSheet = false
                    Task { await self.refresh() }
                },
                onClose: {
                    self.showingSignInSheet = false
                    Task { await self.refresh() }
                }
            )
        }
    }

    @ViewBuilder
    private var statusCard: some View {
        switch self.state {
        case .checking:
            VStack(spacing: 10) {
                ProgressView()
                    .scaleEffect(0.9)
                Text(L10n.tr("setup.cloud_auth.checking"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .frame(maxWidth: .infinity)
            .padding(20)

        case let .authenticated(login, _):
            VStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.secondary)
                Text(L10n.tr("setup.cloud_auth.signed_in"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                if let login {
                    Text("@\(login)")
                        .bobeTextStyle(.helper)
                        .foregroundStyle(self.theme.colors.textMuted)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.secondary.opacity(0.5)))

        case let .unauthenticated(message, _):
            VStack(spacing: 10) {
                Image(systemName: "key.horizontal")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.cloud_auth.signed_out"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(message ?? L10n.tr("setup.cloud_auth.signed_out_hint"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.tertiary.opacity(0.5)))

        case let .error(message):
            VStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.primary)
                Text(L10n.tr("setup.cloud_auth.error"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(message)
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.primary.opacity(0.5)))
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        switch self.state {
        case .checking:
            EmptyView()
        case .authenticated:
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case let .unauthenticated(_, cliPath):
            VStack(spacing: 8) {
                Button(L10n.tr("setup.cloud_auth.sheet.title")) {
                    self.showingSignInSheet = true
                }
                .bobeButton(.primary, size: .regular)
                if self.expertMode.isEnabled {
                    Button(L10n.tr("setup.cloud_auth.open_terminal")) {
                        CopilotSignIn.openLogin(cliPath: cliPath)
                        self.startAuthPolling()
                    }
                    .bobeButton(.secondary, size: .small)
                }
                Button(L10n.tr("setup.cloud_auth.retry")) {
                    Task { await self.refresh() }
                }
                .bobeButton(.secondary, size: .small)
                Button(L10n.tr("setup.cloud_auth.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        case .error:
            VStack(spacing: 8) {
                Button(L10n.tr("setup.cloud_auth.retry")) {
                    Task { await self.refresh() }
                }
                .bobeButton(.primary, size: .regular)
                Button(L10n.tr("setup.cloud_auth.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        }
    }

    private func cardBackground(color: Color) -> some View {
        RoundedRectangle(cornerRadius: 12)
            .fill(color.opacity(0.12))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(color, lineWidth: 1)
            )
    }

    private func refresh() async {
        await MainActor.run { self.state = .checking }
        // Hard ceiling on the initial fetch so a stuck daemon doesn't
        // leave the user staring at a spinner forever. 30s is more than
        // enough for /auth/status on a healthy install; longer than that
        // means something is wrong and the user deserves to know.
        let result = await Self.withTimeout(seconds: 30) {
            try await DaemonClient.shared.getAuthStatus()
        }
        switch result {
        case let .success(resp):
            await MainActor.run {
                if resp.isAuthenticated {
                    self.pollTask?.cancel()
                    self.state = .authenticated(login: resp.login, authType: resp.authType)
                } else {
                    self.state = .unauthenticated(
                        message: resp.statusMessage,
                        cliPath: resp.cliPath
                    )
                }
            }
        case .timedOut:
            await MainActor.run {
                self.state = .error(message: L10n.tr("setup.cloud_auth.error.timeout"))
            }
        case .failed:
            await MainActor.run {
                self.state = .error(message: L10n.tr("setup.cloud_auth.error.generic"))
            }
        }
    }

    /// Generic timeout wrapper. We don't surface the raw error to the UI
    /// (it's almost always a daemon connection hiccup, not something the
    /// user can act on), so we collapse into three discrete outcomes.
    private enum TimedResult<T: Sendable> {
        case success(T)
        case timedOut
        case failed
    }

    private static func withTimeout<T: Sendable>(
        seconds: TimeInterval,
        _ operation: @escaping @Sendable () async throws -> T
    ) async -> TimedResult<T> {
        await withTaskGroup(of: TimedResult<T>.self) { group in
            group.addTask {
                do {
                    let value = try await operation()
                    return .success(value)
                } catch {
                    return .failed
                }
            }
            group.addTask {
                try? await Task.sleep(for: .seconds(seconds))
                return .timedOut
            }
            let first = await group.next() ?? .failed
            group.cancelAll()
            return first
        }
    }

    /// Polls every 2s; caps at 5min so a forgotten Terminal doesn't poll forever.
    private func startAuthPolling() {
        self.pollTask?.cancel()
        self.pollTask = Task { @MainActor in
            let interval = Duration.seconds(2)
            let deadline = ContinuousClock.now + .seconds(300)
            while !Task.isCancelled, ContinuousClock.now < deadline {
                try? await Task.sleep(for: interval)
                if Task.isCancelled { break }
                do {
                    let resp = try await DaemonClient.shared.getAuthStatus()
                    if resp.isAuthenticated {
                        self.state = .authenticated(login: resp.login, authType: resp.authType)
                        return
                    }
                } catch {
                    // CLI may briefly be unresponsive mid-flow; keep polling.
                }
            }
        }
    }
}

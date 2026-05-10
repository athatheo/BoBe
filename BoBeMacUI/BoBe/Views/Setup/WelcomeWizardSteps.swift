import AppKit
import CoreGraphics
import SwiftUI

// MARK: - Welcome step

struct WelcomeStepView: View {
    let onContinue: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            ZStack {
                Circle()
                    .fill(self.theme.colors.primary.opacity(0.15))
                    .frame(width: 88, height: 88)
                Image(systemName: "sparkles")
                    .font(.system(size: 38, weight: .light))
                    .foregroundStyle(self.theme.colors.primary)
            }

            VStack(spacing: 12) {
                Text(L10n.tr("setup.welcome.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("setup.welcome.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }

            Spacer()

            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        }
    }
}

// MARK: - Copilot check step

private enum CopilotCheckState {
    case checking
    case found(path: String)
    case missing
}

struct CopilotCheckStepView: View {
    let onContinue: () -> Void

    @State private var state: CopilotCheckState = .checking
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 20) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.copilot.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.copilot.body"))
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
        .task { await self.runCheck() }
    }

    @ViewBuilder
    private var statusCard: some View {
        switch self.state {
        case .checking:
            HStack(spacing: 10) {
                BobeSpinner(size: 16)
                Text(L10n.tr("setup.copilot.checking"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.border.opacity(0.5)))

        case let .found(path):
            VStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.secondary)
                Text(L10n.tr("setup.copilot.found"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(path)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.secondary.opacity(0.5)))

        case .missing:
            VStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.copilot.missing"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.copilot.missing_hint"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.tertiary.opacity(0.5)))
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        switch self.state {
        case .checking:
            EmptyView()
        case .found:
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case .missing:
            VStack(spacing: 8) {
                HStack(spacing: 10) {
                    Button(L10n.tr("setup.copilot.install_link")) {
                        if let url = URL(string: "https://docs.github.com/en/copilot/cli") {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .bobeButton(.secondary, size: .small)
                    Button(L10n.tr("setup.copilot.retry")) {
                        Task { await self.runCheck() }
                    }
                    .bobeButton(.primary, size: .small)
                    .keyboardShortcut(.defaultAction)
                }
                Button(L10n.tr("setup.copilot.continue_anyway"), action: self.onContinue)
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

    /// Runs `which copilot` to detect Copilot CLI presence. May miss
    /// non-PATH installs — "Continue anyway" is the escape hatch.
    private func runCheck() async {
        self.state = .checking
        let path = await Self.detectCopilotCLI()
        if let path {
            self.state = .found(path: path)
        } else {
            self.state = .missing
        }
    }

    private static func detectCopilotCLI() async -> String? {
        await Task.detached(priority: .userInitiated) { () -> String? in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["which", "copilot"]
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = Pipe()
            do {
                try process.run()
                process.waitUntilExit()
                guard process.terminationStatus == 0 else { return nil }
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let path = String(data: data, encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                return (path?.isEmpty ?? true) ? nil : path
            } catch {
                return nil
            }
        }.value
    }
}

// MARK: - Permissions step

private enum PermissionState {
    case unknown
    case granted
    case denied
}

struct PermissionsStepView: View {
    let onContinue: () -> Void

    @State private var state: PermissionState = .unknown
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 20) {
            VStack(spacing: 8) {
                Text(L10n.tr("setup.permissions.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.permissions.body"))
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
        .onAppear { self.refreshState() }
    }

    @ViewBuilder
    private var statusCard: some View {
        switch self.state {
        case .unknown:
            EmptyView()
        case .granted:
            VStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.secondary)
                Text(L10n.tr("setup.permissions.granted"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.secondary.opacity(0.5)))
        case .denied:
            VStack(spacing: 10) {
                Image(systemName: "lock.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("setup.permissions.denied"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.permissions.denied_hint"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(self.cardBackground(color: self.theme.colors.tertiary.opacity(0.5)))
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        switch self.state {
        case .unknown:
            HStack(spacing: 10) {
                Button(L10n.tr("setup.permissions.grant"), action: self.requestAccess)
                    .bobeButton(.primary, size: .regular)
                    .keyboardShortcut(.defaultAction)
                Button(L10n.tr("setup.permissions.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        case .granted:
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case .denied:
            VStack(spacing: 8) {
                Button(L10n.tr("setup.permissions.open_settings")) {
                    if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                        NSWorkspace.shared.open(url)
                    }
                }
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                Button(L10n.tr("setup.permissions.skip"), action: self.onContinue)
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

    private func refreshState() {
        if CGPreflightScreenCaptureAccess() {
            self.state = .granted
        } else {
            self.state = .unknown
        }
    }

    private func requestAccess() {
        let granted = CGRequestScreenCaptureAccess()
        if granted {
            self.state = .granted
        } else {
            self.state = .denied
        }
    }
}

// MARK: - Done step

struct DoneStepView: View {
    let onLaunch: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            ZStack {
                Circle()
                    .fill(self.theme.colors.secondary.opacity(0.18))
                    .frame(width: 88, height: 88)
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 44, weight: .light))
                    .foregroundStyle(self.theme.colors.secondary)
            }

            VStack(spacing: 12) {
                Text(L10n.tr("setup.done.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(self.theme.colors.text)
                Text(L10n.tr("setup.done.body"))
                    .bobeTextStyle(.setupBody)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }

            Spacer()

            Button(L10n.tr("setup.done.launch"), action: self.onLaunch)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        }
    }
}

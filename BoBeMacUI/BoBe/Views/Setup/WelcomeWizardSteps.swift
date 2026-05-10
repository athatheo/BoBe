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

// MARK: - Engine ready step
//
// Pre-bundling, this step probed `which copilot` and offered install
// instructions if the binary was missing. Now the Copilot CLI is bundled
// into the BoBe daemon binary via the `embedded-cli` cargo feature
// (extracted lazily to ~/.cache/github-copilot-sdk-{ver}/copilot on first
// SDK call). The user never has to install anything, so the probe is
// dead code — this step now just affirms the engine is available.
//
// TODO(phase-2): replace with EngineChoiceStepView (cloud vs local) +
// conditional auth/server-detection step. See ENGINE_PROVIDER_NOTES.md.

struct CopilotCheckStepView: View {
    let onContinue: () -> Void
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

            VStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(self.theme.colors.secondary)
                Text(L10n.tr("setup.copilot.found"))
                    .bobeTextStyle(.setupHeading)
                    .foregroundStyle(self.theme.colors.text)
            }
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(self.theme.colors.secondary.opacity(0.12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(self.theme.colors.secondary.opacity(0.5), lineWidth: 1)
                    )
            )

            Spacer()

            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        }
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

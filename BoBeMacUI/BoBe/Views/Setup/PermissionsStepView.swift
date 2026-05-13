import AppKit
import AVFoundation
import CoreGraphics
import SwiftUI

private enum PermissionState {
    case unknown
    case granted
    case denied
}

struct PermissionsStepView: View {
    let onContinue: () -> Void

    @State private var screenState: PermissionState = .unknown
    @State private var micState: PermissionState = .unknown
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

            HStack(spacing: 12) {
                self.permissionCard(
                    title: "Screen capture",
                    subtitle: "So BoBe sees your context",
                    state: self.screenState,
                    deniedHint: "Enable in System Settings → Privacy → Screen Recording.",
                    settingsURL: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
                )
                self.permissionCard(
                    title: "Microphone",
                    subtitle: "So you can talk to BoBe",
                    state: self.micState,
                    deniedHint: "Enable in System Settings → Privacy → Microphone.",
                    settingsURL: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
                )
            }

            Spacer()

            self.actionRow
        }
        .onAppear { self.refreshState() }
    }

    private func permissionCard(
        title: String,
        subtitle: String,
        state: PermissionState,
        deniedHint: String,
        settingsURL _: String
    ) -> some View {
        VStack(spacing: 10) {
            Image(systemName: self.icon(for: state))
                .font(.system(size: 28))
                .foregroundStyle(self.iconColor(for: state))
            Text(title)
                .bobeTextStyle(.setupHeading)
                .foregroundStyle(self.theme.colors.text)
            Text(self.statusText(for: state, subtitle: subtitle, denied: deniedHint))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity)
        .padding(16)
        .background(self.cardBackground(color: self.iconColor(for: state).opacity(0.5)))
    }

    private func icon(for state: PermissionState) -> String {
        switch state {
        case .unknown: "questionmark.circle.fill"
        case .granted: "checkmark.seal.fill"
        case .denied: "lock.fill"
        }
    }

    private func iconColor(for state: PermissionState) -> Color {
        switch state {
        case .unknown: self.theme.colors.textMuted
        case .granted: self.theme.colors.secondary
        case .denied: self.theme.colors.tertiary
        }
    }

    private func statusText(for state: PermissionState, subtitle: String, denied: String) -> String {
        switch state {
        case .unknown: subtitle
        case .granted: "Allowed"
        case .denied: denied
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        let anyDenied = self.screenState == .denied || self.micState == .denied
        let allGranted = self.screenState == .granted && self.micState == .granted
        if allGranted {
            Button(L10n.tr("setup.welcome.continue"), action: self.onContinue)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        } else if anyDenied {
            VStack(spacing: 8) {
                Button(L10n.tr("setup.permissions.open_settings")) {
                    self.openSettings()
                }
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                Button(L10n.tr("setup.permissions.skip"), action: self.onContinue)
                    .bobeButton(.ghost, size: .small)
            }
        } else {
            HStack(spacing: 10) {
                Button(L10n.tr("setup.permissions.grant"), action: self.requestAccess)
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
        self.screenState = CGPreflightScreenCaptureAccess() ? .granted : .unknown
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: self.micState = .granted
        case .denied, .restricted: self.micState = .denied
        case .notDetermined: self.micState = .unknown
        @unknown default: self.micState = .unknown
        }
    }

    private func requestAccess() {
        // Screen capture is synchronous on macOS; mic is async via
        // AVCaptureDevice. Fire both, update state from each.
        let screenGranted = CGRequestScreenCaptureAccess()
        self.screenState = screenGranted ? .granted : .denied
        Task {
            let micGranted = await AVCaptureDevice.requestAccess(for: .audio)
            await MainActor.run {
                self.micState = micGranted ? .granted : .denied
            }
        }
    }

    private func openSettings() {
        if self.screenState == .denied {
            if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                NSWorkspace.shared.open(url)
            }
        } else if self.micState == .denied {
            if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone") {
                NSWorkspace.shared.open(url)
            }
        }
    }
}

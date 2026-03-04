import AppKit
import SwiftUI

// MARK: - Progress, Capture & Complete Views

extension SetupWizard {
    // MARK: - Setup Progress (job polling)

    var setupProgressView: some View {
        VStack(spacing: 16) {
            Text("Setting up BoBe")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)

            if let job = setupJob {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(job.steps) { step in
                        JobStepRow(step: step)
                    }
                }
            }

            BobeLinearProgressBar(progress: progressPercent / 100)
                .frame(width: 320)

            Text("\(Int(progressPercent))%")
                .font(.system(size: 24, weight: .bold, design: .monospaced))
                .foregroundStyle(theme.colors.primary)

            Text(progressMessage)
                .font(.system(size: 14))
                .foregroundStyle(theme.colors.textMuted)
                .multilineTextAlignment(.center)
                .lineLimit(2)
        }
        .frame(maxWidth: 420)
    }

    // MARK: - Capture Setup

    var captureSetupView: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Screen Awareness")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)

            Text(
                """
                BoBe glances at your screen periodically so it can offer \
                relevant help, track your goals, and remember what you're \
                working on — without you having to explain everything.
                """
            )
            .font(.system(size: 14)).foregroundStyle(theme.colors.textMuted)
            .lineSpacing(2)

            PermissionCard(title: "Screen Recording", badge: screenPermission) {
                Text("Grants BoBe access to see what's on your screen.")
                    .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                if screenPermission == .restricted {
                    Text("This permission is managed by your organization and cannot be changed.")
                        .font(.system(size: 12)).foregroundStyle(theme.colors.tertiary)
                }
                if screenPermission != .granted, screenPermission != .restricted {
                    Button("Open System Settings") {
                        if let url = URL(
                            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
                        ) {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .font(.system(size: 13, weight: .medium)).foregroundStyle(theme.colors.primary)
                    .bobeButton(.ghost, size: .small)
                    Text("Grant permission to continue.")
                        .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                }
            }

            Text(
                setupMode == .local
                    ? "Screenshots never leave your machine — all analysis happens locally."
                    : "Screenshots are sent to the cloud provider you selected for analysis."
            )
            .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted).italic()

            HStack(spacing: 10) {
                Button("Skip — disable screen capture") { skipCapture() }
                    .bobeButton(.secondary, size: .regular)
                    .foregroundStyle(theme.colors.textMuted)
                Spacer()
                Button("Continue") { continueFromCapture() }
                    .bobeButton(.primary, size: .regular)
                    .disabled(screenPermission != .granted)
            }
        }
        .frame(maxWidth: 440)
    }

    // MARK: - Complete

    var completeView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 48))
                .foregroundStyle(theme.colors.secondary)

            Text("Setup Complete!")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)

            VStack(alignment: .leading, spacing: 4) {
                Text("You can always change these settings later.")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                Text(
                    """
                    Click the BoBe icon in your menu bar \u{2192} BoBe Tuning to \
                    manage your AI model, Souls, Goals, screen capture, and \
                    all preferences.
                    """
                )
                .font(.system(size: 14))
                .foregroundStyle(theme.colors.textMuted).lineSpacing(2)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(theme.colors.surface)
                    .stroke(theme.colors.border, lineWidth: 1)
            )

            Button(isFinishingSetup ? "Finishing..." : "Get Started") { completeSetup() }
                .bobeButton(.primary, size: .regular)
                .disabled(isFinishingSetup)
                .padding(.top, 4)
        }
        .frame(maxWidth: 440)
    }
}

// MARK: - Job Step Row

struct JobStepRow: View {
    let step: SetupJobStep
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            self.stepIcon
                .frame(width: 16, height: 16)
            Text(self.step.message ?? self.step.id)
                .font(.system(size: 14, weight: self.step.status == .inProgress ? .semibold : .regular))
                .foregroundStyle(self.stepColor)
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var stepIcon: some View {
        switch self.step.status {
        case .succeeded, .skipped:
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 13))
                .foregroundStyle(self.theme.colors.secondary)
        case .inProgress:
            BobeSpinner(size: 14)
        case .failed:
            Image(systemName: "xmark.circle.fill")
                .font(.system(size: 13))
                .foregroundStyle(self.theme.colors.primary)
        default:
            Image(systemName: "circle")
                .font(.system(size: 13))
                .foregroundStyle(self.theme.colors.textMuted)
        }
    }

    private var stepColor: Color {
        switch self.step.status {
        case .succeeded, .skipped: self.theme.colors.secondary
        case .inProgress: self.theme.colors.text
        case .failed: self.theme.colors.primary
        default: self.theme.colors.textMuted
        }
    }
}

import AppKit
import SwiftUI

extension SetupWizard {
    var setupProgressView: some View {
        VStack(spacing: 16) {
            Text(L10n.tr("setup.progress.title"))
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

    var captureSetupView: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(L10n.tr("setup.capture.title"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)

            Text(L10n.tr("setup.capture.description"))
            .font(.system(size: 14)).foregroundStyle(theme.colors.textMuted)
            .lineSpacing(2)

            PermissionCard(title: L10n.tr("setup.capture.permission.title"), badge: screenPermission) {
                Text(L10n.tr("setup.capture.permission.description"))
                    .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                if screenPermission == .restricted {
                    Text(L10n.tr("setup.capture.permission.restricted"))
                        .font(.system(size: 12)).foregroundStyle(theme.colors.tertiary)
                }
                if screenPermission != .granted, screenPermission != .restricted {
                    Button(L10n.tr("setup.capture.permission.open_settings")) {
                        if let url = URL(
                            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
                        ) {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .font(.system(size: 13, weight: .medium)).foregroundStyle(theme.colors.primary)
                    .bobeButton(.ghost, size: .small)
                    Text(L10n.tr("setup.capture.permission.grant_to_continue"))
                        .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                }
            }

            Text(
                setupMode == .local
                    ? L10n.tr("setup.capture.privacy.local")
                    : L10n.tr("setup.capture.privacy.cloud")
            )
            .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted).italic()

            HStack(spacing: 10) {
                Button(L10n.tr("setup.capture.skip")) { skipCapture() }
                    .bobeButton(.secondary, size: .regular)
                    .foregroundStyle(theme.colors.textMuted)
                Spacer()
                Button(L10n.tr("setup.common.continue")) { continueFromCapture() }
                    .bobeButton(.primary, size: .regular)
                    .disabled(screenPermission != .granted)
            }
        }
        .frame(maxWidth: 440)
    }

    var completeView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 48))
                .foregroundStyle(theme.colors.secondary)

            Text(L10n.tr("setup.complete.title"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)

            VStack(alignment: .leading, spacing: 4) {
                Text(L10n.tr("setup.complete.subtitle"))
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                Text(L10n.tr("setup.complete.details"))
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

            Button(isFinishingSetup ? L10n.tr("setup.complete.finishing") : L10n.tr("setup.complete.get_started")) { completeSetup() }
                .bobeButton(.primary, size: .regular)
                .disabled(isFinishingSetup)
                .padding(.top, 4)
        }
        .frame(maxWidth: 440)
    }
}

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

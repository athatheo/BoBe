import AppKit
import SwiftUI

// MARK: - Download, Capture & Complete Views

extension SetupWizard {

    // MARK: - Downloading

    var downloadingView: some View {
        VStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 0) {
                StepIndicator(
                    label: "Downloading AI engine",
                    active: step == .downloadingEngine,
                    done: step == .downloadingModel || step == .initializing
                )
                StepIndicator(
                    label: "Downloading language model",
                    active: step == .downloadingModel,
                    done: step == .initializing
                )
                StepIndicator(
                    label: "Initializing BoBe",
                    active: step == .initializing,
                    done: false
                )
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

            Text("""
                BoBe glances at your screen periodically so it can offer \
                relevant help, track your goals, and remember what you're \
                working on — without you having to explain everything.
                """)
                .font(.system(size: 14)).foregroundStyle(theme.colors.textMuted)
                .lineSpacing(2)

            PermissionCard(title: "Screen Recording", badge: screenPermission) {
                Text("Grants BoBe access to see what's on your screen.")
                    .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                if screenPermission == "restricted" {
                    Text("This permission is managed by your organization and cannot be changed.")
                        .font(.system(size: 12)).foregroundStyle(theme.colors.tertiary)
                }
                if screenPermission != "granted" && screenPermission != "restricted" {
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

            if setupMode == .local {
                PermissionCard(title: "Vision Model", badge: visionBadge) {
                    Text("\(selectedModelOption.visionModel) for screen analysis.")
                        .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                    if visionDownloading {
                        BobeLinearProgressBar(progress: visionProgress / 100)
                        Text(visionMessage)
                            .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted)
                    }
                    if !visionError.isEmpty {
                        Text(visionError)
                            .font(.system(size: 12)).foregroundStyle(theme.colors.primary)
                        Button("Retry download") { startVisionDownload() }
                            .font(.system(size: 13, weight: .medium)).foregroundStyle(theme.colors.primary)
                            .bobeButton(.ghost, size: .small)
                    }
                }
            }

            Text(setupMode == .local
                ? "Screenshots never leave your machine — all analysis happens locally."
                : "Screenshots are sent to the cloud provider you selected for analysis.")
                .font(.system(size: 12)).foregroundStyle(theme.colors.textMuted).italic()

            HStack(spacing: 10) {
                Button("Skip — disable screen capture") { skipCapture() }
                    .bobeButton(.secondary, size: .regular)
                    .foregroundStyle(theme.colors.textMuted)
                Spacer()
                let visionReady = setupMode != .local || visionDownloaded
                let canContinue = screenPermission == "granted" && visionReady
                Button("Continue") { continueFromCapture() }
                    .bobeButton(.primary, size: .regular)
                    .disabled(!canContinue)
            }
        }
        .frame(maxWidth: 440)
    }

    var visionBadge: String {
        if visionDownloaded { return "granted" }
        if visionDownloading { return "not-determined" }
        if !visionError.isEmpty { return "denied" }
        return "not-determined"
    }

    // MARK: - Complete

    var completeView: some View {
        VStack(spacing: 16) {
            Text("You're all set!")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)

            VStack(spacing: 6) {
                SummaryRow(
                    label: "AI Model",
                    value: setupMode == .online ? "Cloud LLM" : selectedModelOption.label,
                    ok: true
                )
                SummaryRow(
                    label: "Screen Capture",
                    value: captureSkipped ? "Disabled (skipped)" : "Enabled",
                    ok: !captureSkipped
                )
            }

            VStack(alignment: .leading, spacing: 4) {
                Text("You can always change these settings later.")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                Text("""
                    Click the BoBe icon in your menu bar \u{2192} BoBe Tuning to \
                    manage your AI model, Souls, Goals, screen capture, and \
                    all preferences.
                    """)
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

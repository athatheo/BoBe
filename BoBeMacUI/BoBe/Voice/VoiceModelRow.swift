import SwiftUI

/// Single voice-model status row used by both the welcome wizard and
/// Settings → Voice. Lives at the Voice/ feature root because both
/// surfaces consume it through the same `VoiceModelProgress` DTO.
struct VoiceModelRow: View {
    let model: VoiceModelProgress
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 12) {
            VoiceModelStatusIcon(status: model.status)
                .frame(width: 18, height: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(self.model.label.capitalized)
                    .font(.system(size: 13))
                    .foregroundStyle(self.theme.colors.text)
                Text(self.localizedStatus)
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            Spacer()
            if let percent = self.model.percent {
                Text("\(percent)%")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(self.model.label.capitalized): \(self.localizedStatus)")
    }

    /// Map daemon-side status strings to localized labels. Daemon strings come
    /// from `voice/install_service.rs::ModelProgress::{pending,downloading,
    /// installed,already installed,failed,canceled}`.
    private var localizedStatus: String {
        switch self.model.status {
        case "installed": L10n.tr("voice.model.status.installed")
        case "already installed": L10n.tr("voice.model.status.already_installed")
        case "downloading": L10n.tr("voice.model.status.downloading")
        case "pending": L10n.tr("voice.model.status.pending")
        case "failed": L10n.tr("voice.model.status.failed")
        case "canceled": L10n.tr("voice.model.status.canceled")
        default: self.model.status
        }
    }
}

/// 18x18 status indicator — checkmark for installed, spinner for downloading,
/// dotted circle otherwise. Rendered inside `VoiceModelRow` and reused in any
/// future per-model UI.
struct VoiceModelStatusIcon: View {
    let status: String
    @Environment(\.theme) private var theme

    var body: some View {
        Group {
            switch self.status {
            case "installed", "already installed":
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(self.theme.colors.secondary)
            case "downloading":
                ProgressView()
                    .progressViewStyle(.circular)
                    .controlSize(.small)
            default:
                Image(systemName: "circle.dotted")
                    .foregroundStyle(self.theme.colors.textMuted.opacity(0.6))
            }
        }
        .accessibilityHidden(true)
    }
}

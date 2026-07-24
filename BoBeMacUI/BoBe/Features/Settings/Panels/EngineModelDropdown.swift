import SwiftUI

extension EnginePanel {
    func modelDropdown(
        label: String,
        description: String,
        keyPath: WritableKeyPath<DaemonSettings, String?>,
        reasoningKeyPath: WritableKeyPath<DaemonSettings, String?>,
        visionOnly: Bool
    ) -> some View {
        let pool = visionOnly ? self.availableModels.filter(\.vision) : self.availableModels
        // "—" sentinel means "use the CLI's default model" — daemon's resolver picks cheapest available.
        let options = ["—"] + pool.map(\.id)
        let displayName = { (id: String) -> String in
            if id == "—" {
                return L10n.tr("settings.engine.models.use_default")
            }
            guard let model = self.availableModels.first(where: { $0.id == id }) else { return id }
            if let mult = model.multiplier {
                return "\(model.name)  ·  \(Self.formatMultiplier(mult))"
            }
            return model.name
        }

        let modelBinding = Binding<String>(
            get: {
                let raw = self.store.settings?[keyPath: keyPath]
                if let raw, !raw.isEmpty { return raw }
                return "—"
            },
            set: { newValue in
                let availableModels = self.availableModels
                self.store.update { current in
                    // Empty string is the clear sentinel; daemon normalizes "" back to None.
                    current[keyPath: keyPath] = (newValue == "—") ? "" : newValue
                    let supported = availableModels
                        .first(where: { $0.id == newValue })?.supportedReasoningEfforts ?? []
                    current[keyPath: reasoningKeyPath] = Self.retainedReasoningEffort(
                        current[keyPath: reasoningKeyPath],
                        supported: supported
                    )
                }
            }
        )

        let selectedModel = self.availableModels.first { $0.id == self.store.settings?[keyPath: keyPath] }

        return VStack(alignment: .leading, spacing: 8) {
            SettingsRow(label: label, description: description) {
                BobeMenuPicker(
                    selection: modelBinding,
                    options: options,
                    label: displayName,
                    width: 280
                )
            }
            if let model = selectedModel, model.supportsReasoningEffort {
                self.reasoningRow(model: model, keyPath: reasoningKeyPath)
            }
        }
    }

    static func retainedReasoningEffort(_ current: String?, supported: [String]) -> String {
        guard let current, !current.isEmpty, supported.contains(current) else {
            return ""
        }
        return current
    }

    @ViewBuilder
    func reasoningRow(model: ModelInfo, keyPath: WritableKeyPath<DaemonSettings, String?>) -> some View {
        let efforts = model.supportedReasoningEfforts
        let defaultLabel = L10n.tr("settings.engine.reasoning.use_default")
        let options = ["—"] + efforts

        let binding = Binding<String>(
            get: {
                let raw = self.store.settings?[keyPath: keyPath]
                if let raw, !raw.isEmpty, efforts.contains(raw) { return raw }
                return "—"
            },
            set: { newValue in
                self.store.update { current in
                    current[keyPath: keyPath] = (newValue == "—") ? "" : newValue
                }
            }
        )

        let displayName = { (id: String) -> String in
            if id == "—" {
                if let def = model.defaultReasoningEffort {
                    return "\(defaultLabel) (\(def))"
                }
                return defaultLabel
            }
            return id.capitalized
        }

        HStack(spacing: 8) {
            Image(systemName: "brain")
                .font(.system(size: 11))
                .foregroundStyle(self.theme.colors.tertiary)
            Text(L10n.tr("settings.engine.reasoning.label"))
                .bobeTextStyle(.body)
                .foregroundStyle(self.theme.colors.textMuted)
            BobeMenuPicker(
                selection: binding,
                options: options,
                label: displayName,
                width: 220
            )
        }
        .padding(.leading, 8)
    }

    /// "0×" → free, "1×" → base, "0.33×" → cheap, "15×" → 15x base rate.
    static func formatMultiplier(_ value: Double) -> String {
        if value == 0 { return "0×" }
        if value == value.rounded() {
            return "\(Int(value))×"
        }
        // Two decimals when fractional (e.g., 0.33×, 7.5× becomes "7.50×" — trim trailing zero).
        let s = String(format: "%.2f", value)
        let trimmed = s.hasSuffix("0") ? String(s.dropLast()) : s
        return "\(trimmed)×"
    }
}

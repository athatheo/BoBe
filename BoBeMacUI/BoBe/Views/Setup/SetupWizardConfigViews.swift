import SwiftUI

extension SetupWizard {
    var cloudConfigView: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("setup.cloud.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(theme.colors.text)
                    .frame(maxWidth: .infinity, alignment: .center)

                if let providers = options?.cloudProviders, !providers.isEmpty {
                    Text(L10n.tr("setup.cloud.provider"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeMenuPicker(
                        selection: $selectedProvider,
                        options: providers.map(\.id),
                        label: { providerId in
                            providers.first(where: { $0.id == providerId })?.label ?? providerId
                        },
                        width: 440
                    )
                    .accessibilityLabel(L10n.tr("setup.cloud.provider"))
                    .onChange(of: selectedProvider) { _, newProvider in
                        let provider = providers.first(where: { $0.id == newProvider })
                        selectedModel = provider?.models.first?.id ?? ""
                        apiKey = ""
                        endpoint = ""
                        deployment = ""
                    }
                }

                Text(L10n.tr("setup.cloud.api_key"))
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(theme.colors.text)
                BobeSecureField(placeholder: L10n.tr("setup.cloud.api_key.placeholder"), text: $apiKey)

                if let provider = options?.cloudProviders.first(where: { $0.id == selectedProvider }),
                   provider.needsEndpoint {
                    Text(L10n.tr("setup.cloud.endpoint"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: L10n.tr("setup.cloud.endpoint.placeholder"), text: $endpoint)
                }

                if let provider = options?.cloudProviders.first(where: { $0.id == selectedProvider }),
                   provider.needsDeployment {
                    Text(L10n.tr("setup.cloud.deployment"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: L10n.tr("setup.cloud.deployment.placeholder"), text: $deployment)
                }

                if let provider = options?.cloudProviders.first(where: { $0.id == selectedProvider }),
                   !provider.models.isEmpty {
                    Text(L10n.tr("setup.cloud.model"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeMenuPicker(
                        selection: $selectedModel,
                        options: provider.models.map(\.id),
                        label: { modelId in
                            provider.models.first(where: { $0.id == modelId })?.label ?? modelId
                        },
                        width: 440
                    )
                    .accessibilityLabel(L10n.tr("setup.cloud.model"))
                }

                let provider = options?.cloudProviders.first { $0.id == selectedProvider }
                let needsEndpoint = provider?.needsEndpoint ?? false
                let needsDeployment = provider?.needsDeployment ?? false
                let canSubmit =
                    !apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        && (!needsEndpoint || !endpoint.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        && (!needsDeployment || !deployment.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        && !busy

                Button(busy ? L10n.tr("setup.common.setting_up") : L10n.tr("setup.common.continue")) {
                    handleCloudSetup()
                }
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
                .disabled(!canSubmit)
                .frame(maxWidth: .infinity)

                self.backToChooseModeButton {
                    apiKey = ""
                    endpoint = ""
                    deployment = ""
                }
            }
            .frame(maxWidth: 440)
        }
    }

    var localConfigView: some View {
        VStack(alignment: .leading, spacing: 16) {
                Text(L10n.tr("setup.local.title"))
                    .bobeTextStyle(.setupTitle)
                    .foregroundStyle(theme.colors.text)
                    .frame(maxWidth: .infinity, alignment: .center)

            Text(L10n.tr("setup.local.subtitle"))
                .bobeTextStyle(.setupSubtitle)
                .foregroundStyle(theme.colors.textMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)

            if let tiers = options?.localTiers {
                ForEach(tiers) { tier in
                    TierCard(tier: tier, isSelected: selectedTier == tier.id) {
                        selectedTier = tier.id
                    }
                }
            }

            Text(L10n.tr("setup.local.change_later_hint"))
                .font(.system(size: 12))
                .foregroundStyle(theme.colors.textMuted)
                .frame(maxWidth: .infinity, alignment: .center)

            Button(busy ? L10n.tr("setup.common.setting_up") : L10n.tr("setup.common.continue")) {
                handleLocalSetup()
            }
            .bobeButton(.primary, size: .regular)
            .keyboardShortcut(.defaultAction)
            .disabled(busy)
            .frame(maxWidth: .infinity)

            self.backToChooseModeButton()
        }
        .frame(maxWidth: 440)
    }

    func backToChooseModeButton(cleanup: (() -> Void)? = nil) -> some View {
        Button {
            cleanup?()
            step = .chooseMode
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "chevron.left")
                    .font(.system(size: 12))
                Text(L10n.tr("setup.common.back"))
                    .font(.system(size: 14))
            }
            .foregroundStyle(theme.colors.textMuted)
        }
        .bobeButton(.ghost, size: .small)
        .keyboardShortcut(.cancelAction)
    }
}

struct TierCard: View {
    let tier: LocalTier
    let isSelected: Bool
    let onSelect: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        Button(action: self.onSelect) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: self.isSelected ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(self.isSelected ? self.theme.colors.primary : self.theme.colors.border)
                    .font(.system(size: 14))
                    .padding(.top, 2)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(self.tier.label)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(self.theme.colors.text)
                        Text(self.tier.diskLabel)
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    Text(self.tier.description)
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                Spacer()
            }
            .padding(10)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.isSelected ? self.theme.colors.primary.opacity(0.12) : self.theme.colors.surface)
                    .stroke(self.isSelected ? self.theme.colors.primary : self.theme.colors.border, lineWidth: 1)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(self.isSelected ? .isSelected : [])
    }
}

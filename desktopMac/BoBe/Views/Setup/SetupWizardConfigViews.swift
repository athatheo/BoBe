import AppKit
import SwiftUI

// MARK: - Cloud & Local Config Views

extension SetupWizard {

    // MARK: - Cloud Config

    var cloudConfigView: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 16) {
                Text("Cloud AI Setup")
                    .font(.system(size: 26, weight: .bold))
                    .foregroundStyle(theme.colors.text)
                    .frame(maxWidth: .infinity, alignment: .center)

                VStack(alignment: .leading, spacing: 10) {
                    Text("OpenAI API Key")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeSecureField(placeholder: "sk-...", text: $apiKey)

                    Text("Model")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeMenuPicker(
                        selection: $selectedOpenAIModel,
                        options: openAIModelOptions.map(\.modelName),
                        label: { modelName in
                            openAIModelOptions.first(where: { $0.modelName == modelName })?.label ?? modelName
                        }
                    )

                    Button(busy ? "Configuring..." : "Continue with OpenAI") {
                        handleChooseOpenAI()
                    }
                    .bobeButton(.primary, size: .regular)
                    .disabled(apiKey.trimmingCharacters(in: .whitespaces).isEmpty || busy)
                    .frame(maxWidth: .infinity)
                }

                SetupCollapsibleSection(
                    title: "Use another cloud provider",
                    collapsedTitle: "Hide cloud options",
                    isExpanded: $showAzure
                ) {
                    Text("Provider")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeMenuPicker(
                        selection: $onlineProvider,
                        options: onlineProviders.filter { $0.id != "openai" }.map(\.id),
                        label: { providerId in
                            onlineProviders.first(where: { $0.id == providerId })?.label ?? providerId
                        }
                    )
                    .onChange(of: onlineProvider) { _, newProvider in
                        onlineModel = onlineProviders.first(where: { $0.id == newProvider })?.defaultModel ?? ""
                        endpoint = ""
                    }

                    Text("API Key")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeSecureField(
                        placeholder: onlineProviders.first { $0.id == onlineProvider }?.placeholder ?? "API Key",
                        text: $apiKey
                    )

                    if let p = onlineProviders.first(where: { $0.id == onlineProvider }),
                       p.needsEndpoint {
                        Text("Endpoint URL")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(theme.colors.text)
                        BobeTextField(placeholder: p.endpointPlaceholder, text: $endpoint)
                    }

                    Text("Model")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: "Model name", text: $onlineModel)

                    let provider = onlineProviders.first { $0.id == onlineProvider }
                    let needsEndpoint = provider?.needsEndpoint ?? false
                    let canSubmit = !apiKey.trimmingCharacters(in: .whitespaces).isEmpty
                        && (!needsEndpoint || !endpoint.trimmingCharacters(in: .whitespaces).isEmpty)
                        && !busy

                    Button(busy ? "Configuring..." : "Continue with cloud LLM") {
                        handleChooseOnline()
                    }
                    .bobeButton(.primary, size: .regular)
                    .disabled(!canSubmit)
                    .frame(maxWidth: .infinity)
                }

                backToChooseModeButton {
                    apiKey = ""
                    endpoint = ""
                    onlineModel = onlineProviders.first { $0.id == "azure_openai" }?.defaultModel ?? ""
                }
            }
            .frame(maxWidth: 440)
        }
    }

    // MARK: - Local Config

    var localConfigView: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Local AI Setup")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)
                .frame(maxWidth: .infinity, alignment: .center)

            Text("Download and run AI entirely on your Mac. No internet needed after setup.")
                .font(.system(size: 15))
                .foregroundStyle(theme.colors.textMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)

            ForEach(localModelOptions) { model in
                ThemedModelCard(model: model, isSelected: selectedLocalModel == model.id) {
                    selectedLocalModel = model.id
                }
            }

            Button(busy ? "Checking connection..." : "Continue") {
                handleChooseLocal()
            }
            .bobeButton(.primary, size: .regular)
            .disabled(busy)
            .frame(maxWidth: .infinity)

            backToChooseModeButton()
        }
        .frame(maxWidth: 440)
    }

    // MARK: - Shared Components

    func backToChooseModeButton(cleanup: (() -> Void)? = nil) -> some View {
        Button {
            cleanup?()
            step = .chooseMode
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "chevron.left")
                    .font(.system(size: 12))
                Text("Back")
                    .font(.system(size: 14))
            }
            .foregroundStyle(theme.colors.textMuted)
        }
        .bobeButton(.ghost, size: .small)
    }
}

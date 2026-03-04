import SwiftUI

/// AI Model settings panel — provider picker (Ollama/OpenAI/Azure) with model management.
struct AIModelPanel: View {
    @State private var settings: DaemonSettings?
    @State private var selectedProvider = "ollama"
    @State private var ollamaModel = ""
    @State private var openaiModel = "gpt-5-mini"
    @State private var openaiApiKey = ""
    @State private var azureEndpoint = ""
    @State private var azureDeployment = ""
    @State private var azureApiKey = ""
    @State private var onboardingOptions: OnboardingOptions?
    @State private var models: [ModelInfo] = []
    @State private var pullModelName = ""
    @State private var isPulling = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var isDirty = false
    @State private var error: String?
    @Environment(\.theme) private var theme

    private var openAIProvider: CloudProvider? {
        self.onboardingOptions?.cloudProviders.first(where: { $0.id == "openai" })
    }

    private var openAIModelChoices: [String] {
        var choices = self.openAIProvider?.models.map(\.id) ?? []
        if !self.openaiModel.isEmpty, !choices.contains(self.openaiModel) {
            choices.insert(self.openaiModel, at: 0)
        }
        return choices
    }

    var body: some View {
        ZStack(alignment: .top) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    if let error {
                        HStack(spacing: 6) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(self.theme.colors.primary)
                            Text(error)
                                .font(.system(size: 12))
                                .foregroundStyle(self.theme.colors.primary)
                        }
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 8).fill(self.theme.colors.primary.opacity(0.08)))
                    }

                    if self.isLoading, self.settings == nil {
                        HStack(spacing: 8) {
                            BobeSpinner(size: 14)
                            Text("Loading model settings...")
                                .font(.system(size: 13))
                                .foregroundStyle(self.theme.colors.textMuted)
                        }
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.top, 40)
                    } else if self.settings != nil {
                        SettingsRow(label: "Provider") {
                            BobeMenuPicker(
                                selection: self.$selectedProvider,
                                options: ["ollama", "openai", "azure_openai"],
                                label: { provider in
                                    switch provider {
                                    case "ollama": "Ollama (Local)"
                                    case "openai": "OpenAI"
                                    case "azure_openai": "Azure OpenAI"
                                    default: provider
                                    }
                                },
                                width: 200
                            )
                            .onChange(of: self.selectedProvider) { _, _ in self.isDirty = true }
                        }

                        Divider()

                        switch self.selectedProvider {
                        case "ollama": self.ollamaSettings
                        case "openai": self.openaiSettings
                        case "azure_openai": self.azureSettings
                        default: EmptyView()
                        }
                    }
                }
                .padding(24)
                .padding(.top, self.isDirty ? 48 : 0)
            }

            if self.isDirty {
                HStack(spacing: 12) {
                    Text("Unsaved changes")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    Spacer()
                    Button("Discard") { self.discardChanges() }
                        .bobeButton(.secondary, size: .small)
                    Button(self.isSaving ? "Saving..." : "Save") { self.saveSettings() }
                        .bobeButton(.primary, size: .small)
                        .disabled(self.isSaving)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(
                    Rectangle()
                        .fill(self.theme.colors.surface)
                        .shadow(color: .black.opacity(0.1), radius: 4, y: 2)
                )
                .transition(.move(edge: .top).combined(with: .opacity))
                .animation(.easeOut(duration: 0.2), value: self.isDirty)
            }
        }
        .task { await self.loadSettings() }
    }

    private var ollamaSettings: some View {
        VStack(alignment: .leading, spacing: 16) {
            CollapsibleSection(
                title: "Active Model",
                icon: "cpu",
                description: "The model BoBe uses for all AI operations"
            ) {
                if self.models.isEmpty {
                    Text("No models installed — is Ollama running?")
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                } else {
                    BobeMenuPicker(
                        selection: self.$ollamaModel,
                        options: self.models.map(\.name),
                        label: { $0 },
                        width: 260
                    )
                    .onChange(of: self.ollamaModel) { _, _ in self.isDirty = true }
                }
            }

            if !self.models.isEmpty {
                CollapsibleSection(
                    title: "Installed Models",
                    icon: "tray.full.fill",
                    description: "\(self.models.count) models downloaded"
                ) {
                    ForEach(self.models) { model in
                        HStack(spacing: 8) {
                            Text(model.name)
                                .font(.system(size: 13, weight: .medium))

                            if model.name == self.ollamaModel {
                                Text("Active")
                                    .font(.system(size: 9, weight: .bold))
                                    .foregroundStyle(self.theme.colors.secondary)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(Capsule().fill(self.theme.colors.secondary.opacity(0.15)))
                            }

                            Spacer()

                            Text(formatBytes(model.sizeBytes))
                                .font(.system(size: 11))
                                .foregroundStyle(self.theme.colors.textMuted)

                            Button("Use") {
                                self.ollamaModel = model.name
                                self.isDirty = true
                            }
                            .bobeButton(.secondary, size: .mini)

                            Button {
                                Task { await self.deleteModel(model.name) }
                            } label: {
                                Image(systemName: "trash")
                            }
                            .bobeButton(.destructive, size: .mini)
                        }
                        .padding(.vertical, 3)
                    }
                }
            }

            CollapsibleSection(
                title: "Download Model",
                icon: "arrow.down.circle.fill",
                description: "Pull a model from the Ollama registry"
            ) {
                if self.isPulling {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Downloading \(self.pullModelName)...")
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                } else {
                    HStack(spacing: 8) {
                        BobeTextField(placeholder: "Model name (e.g. llama3.2)", text: self.$pullModelName) {
                            if !self.pullModelName.isEmpty { Task { await self.pullModel() } }
                        }
                        Button("Pull") {
                            Task { await self.pullModel() }
                        }
                        .bobeButton(.secondary, size: .small)
                        .disabled(self.pullModelName.isEmpty)
                    }
                }
            }
        }
    }

    private var openaiSettings: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("OpenAI Configuration")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(self.theme.colors.text)

            SettingsRow(label: "API Key") {
                BobeSecureField(
                    placeholder: self.openaiApiKey.isEmpty ? "sk-..." : "••••••••",
                    text: self.$openaiApiKey,
                    width: 280
                )
                .onChange(of: self.openaiApiKey) { _, _ in self.isDirty = true }
            }
            SettingsRow(label: "Model") {
                BobeMenuPicker(
                    selection: self.$openaiModel,
                    options: self.openAIModelChoices,
                    label: { modelId in
                        self.openAIProvider?.models.first(where: { $0.id == modelId })?.label ?? modelId
                    },
                    width: 220
                )
                .onChange(of: self.openaiModel) { _, _ in self.isDirty = true }
            }
        }
    }

    private var azureSettings: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Azure OpenAI Configuration")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(self.theme.colors.text)

            SettingsRow(label: "Endpoint") {
                BobeTextField(placeholder: "https://...", text: self.$azureEndpoint, width: 280)
                    .onChange(of: self.azureEndpoint) { _, _ in self.isDirty = true }
            }
            SettingsRow(label: "API Key") {
                BobeSecureField(
                    placeholder: self.azureApiKey.isEmpty ? "key" : "••••••••",
                    text: self.$azureApiKey,
                    width: 280
                )
                .onChange(of: self.azureApiKey) { _, _ in self.isDirty = true }
            }
            SettingsRow(label: "Deployment") {
                BobeTextField(placeholder: "deployment-name", text: self.$azureDeployment, width: 200)
                    .onChange(of: self.azureDeployment) { _, _ in self.isDirty = true }
            }
        }
    }

    // MARK: - Actions

    private func loadSettings() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            let s = try await DaemonClient.shared.getSettings()
            self.settings = s
            self.selectedProvider = ["ollama", "openai", "azure_openai"].contains(s.llmBackend) ? s.llmBackend : "ollama"
            self.ollamaModel = s.ollamaModel
            self.openaiModel = s.openaiModel
            self.azureEndpoint = s.azureOpenaiEndpoint
            self.azureDeployment = s.azureOpenaiDeployment

            self.onboardingOptions = try? await DaemonClient.shared.getOnboardingOptions()
            let modelsResp = try await DaemonClient.shared.listModels()
            self.models = modelsResp.models
            self.isDirty = false
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func saveSettings() {
        self.isSaving = true
        Task {
            defer { isSaving = false }
            do {
                var req = SettingsUpdateRequest()
                req.llmBackend = self.selectedProvider
                req.ollamaModel = self.ollamaModel
                req.openaiModel = self.openaiModel
                if !self.openaiApiKey.isEmpty { req.openaiApiKey = self.openaiApiKey }
                req.azureOpenaiEndpoint = self.azureEndpoint
                req.azureOpenaiDeployment = self.azureDeployment
                if !self.azureApiKey.isEmpty { req.azureOpenaiApiKey = self.azureApiKey }
                _ = try await DaemonClient.shared.updateSettings(req)
                self.isDirty = false
                self.error = nil
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func discardChanges() {
        guard let s = settings else { return }
        self.selectedProvider = ["ollama", "openai", "azure_openai"].contains(s.llmBackend) ? s.llmBackend : "ollama"
        self.ollamaModel = s.ollamaModel
        self.openaiModel = s.openaiModel
        self.azureEndpoint = s.azureOpenaiEndpoint
        self.azureDeployment = s.azureOpenaiDeployment
        self.openaiApiKey = ""
        self.azureApiKey = ""
        self.isDirty = false
    }

    private func pullModel() async {
        self.isPulling = true
        defer { isPulling = false }
        do {
            try await DaemonClient.shared.pullModel(self.pullModelName)
            self.pullModelName = ""
            let resp = try await DaemonClient.shared.listModels()
            self.models = resp.models
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func deleteModel(_ name: String) async {
        do {
            try await DaemonClient.shared.deleteModel(name)
            let resp = try await DaemonClient.shared.listModels()
            self.models = resp.models
        } catch {
            self.error = error.localizedDescription
        }
    }
}

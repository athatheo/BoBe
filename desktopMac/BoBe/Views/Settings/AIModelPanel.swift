import SwiftUI

/// AI Model settings panel — provider picker (Ollama/OpenAI/Azure) with model management.
/// Based on AIModelSettings.tsx with save bar, model dropdown, pull progress.
struct AIModelPanel: View {
    @State private var settings: DaemonSettings?
    @State private var selectedProvider = "ollama"
    @State private var ollamaModel = ""
    @State private var openaiModel = "gpt-4o-mini"
    @State private var openaiApiKey = ""
    @State private var azureEndpoint = ""
    @State private var azureDeployment = ""
    @State private var azureApiKey = ""
    @State private var models: [ModelInfo] = []
    @State private var pullModelName = ""
    @State private var isPulling = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var isDirty = false
    @State private var error: String?
    @Environment(\.theme) private var theme
    private let openAIModelOptions = ["gpt-5-mini", "gpt-4o-mini", "gpt-4.1-mini", "gpt-4.1", "o4-mini"]
    private let popularOllamaModels = ["qwen3-vl:2b", "qwen3-vl:4b", "qwen3:14b", "llama3.2", "gemma3:4b", "phi4-mini"]

    private var openAIModelChoices: [String] {
        var models = openAIModelOptions
        if !openaiModel.isEmpty && !models.contains(openaiModel) {
            models.insert(openaiModel, at: 0)
        }
        return models
    }

    var body: some View {
        ZStack(alignment: .top) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Error banner
                    if let error {
                        HStack(spacing: 6) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(theme.colors.primary)
                            Text(error)
                                .font(.system(size: 12))
                                .foregroundStyle(theme.colors.primary)
                        }
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 8).fill(theme.colors.primary.opacity(0.08)))
                    }

                    if isLoading && settings == nil {
                        HStack(spacing: 8) {
                            BobeSpinner(size: 14)
                            Text("Loading model settings...")
                                .font(.system(size: 13))
                                .foregroundStyle(theme.colors.textMuted)
                        }
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.top, 40)
                    } else if settings != nil {
                        // Provider picker
                        SettingsRow(label: "Provider") {
                            BobeMenuPicker(
                                selection: $selectedProvider,
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
                            .onChange(of: selectedProvider) { _, _ in isDirty = true }
                        }

                        Divider()

                        // Provider-specific settings
                        switch selectedProvider {
                        case "ollama": ollamaSettings
                        case "openai": openaiSettings
                        case "azure_openai": azureSettings
                        default: EmptyView()
                        }
                    }
                }
                .padding(24)
                .padding(.top, isDirty ? 48 : 0)
            }

            // Save bar (appears when dirty)
            if isDirty {
                HStack(spacing: 12) {
                    Text("Unsaved changes")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    Spacer()
                    Button("Discard") { discardChanges() }
                        .bobeButton(.secondary, size: .small)
                    Button(isSaving ? "Saving..." : "Save") { saveSettings() }
                        .bobeButton(.primary, size: .small)
                        .disabled(isSaving)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(
                    Rectangle()
                        .fill(theme.colors.surface)
                        .shadow(color: .black.opacity(0.1), radius: 4, y: 2)
                )
                .transition(.move(edge: .top).combined(with: .opacity))
                .animation(.easeOut(duration: 0.2), value: isDirty)
            }
        }
        .task { await loadSettings() }
    }

    private var ollamaSettings: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Active model
            CollapsibleSection(
                title: "Active Model",
                icon: "cpu",
                description: "The model BoBe uses for all AI operations"
            ) {
                if models.isEmpty {
                    Text("No models installed — is Ollama running?")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.textMuted)
                } else {
                    BobeMenuPicker(
                        selection: $ollamaModel,
                        options: models.map(\.name),
                        label: { $0 },
                        width: 260
                    )
                    .onChange(of: ollamaModel) { _, _ in isDirty = true }
                }
            }

            // Installed models
            if !models.isEmpty {
                CollapsibleSection(
                    title: "Installed Models",
                    icon: "tray.full.fill",
                    description: "\(models.count) models downloaded"
                ) {
                    ForEach(models) { model in
                        HStack(spacing: 8) {
                            Text(model.name)
                                .font(.system(size: 13, weight: .medium))

                            if model.name == ollamaModel {
                                Text("Active")
                                    .font(.system(size: 9, weight: .bold))
                                    .foregroundStyle(theme.colors.secondary)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(Capsule().fill(theme.colors.secondary.opacity(0.15)))
                            }

                            Spacer()

                            Text(formatBytes(model.sizeBytes))
                                .font(.system(size: 11))
                                .foregroundStyle(theme.colors.textMuted)

                            Button("Use") {
                                ollamaModel = model.name
                                isDirty = true
                            }
                            .bobeButton(.secondary, size: .mini)

                            Button {
                                Task { await deleteModel(model.name) }
                            } label: {
                                Image(systemName: "trash")
                            }
                            .bobeButton(.destructive, size: .mini)
                        }
                        .padding(.vertical, 3)
                    }
                }
            }

            // Download model
            CollapsibleSection(
                title: "Download Model",
                icon: "arrow.down.circle.fill",
                description: "Pull a model from the Ollama registry"
            ) {
                if isPulling {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Downloading \(pullModelName)...")
                            .font(.system(size: 12))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                } else {
                    HStack(spacing: 8) {
                        BobeTextField(placeholder: "Model name (e.g. llama3.2)", text: $pullModelName) {
                            if !pullModelName.isEmpty { Task { await pullModel() } }
                        }
                        Button("Pull") {
                            Task { await pullModel() }
                        }
                        .bobeButton(.secondary, size: .small)
                        .disabled(pullModelName.isEmpty)
                    }

                    VStack(alignment: .leading, spacing: 6) {
                        Text("Popular Ollama models")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(theme.colors.textMuted)
                        FlowLayout(spacing: 6) {
                            ForEach(popularOllamaModels, id: \.self) { model in
                                Button(model) {
                                    pullModelName = model
                                }
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(theme.colors.text)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 5)
                                .background(
                                    Capsule()
                                        .fill(theme.colors.surface)
                                        .overlay(
                                            Capsule()
                                                .stroke(theme.colors.border, lineWidth: 1)
                                        )
                                )
                                .buttonStyle(.plain)
                            }
                        }
                        Text("Click a model to fill the field, then pull.")
                            .font(.system(size: 10))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                }
            }
        }
    }

    private var openaiSettings: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("OpenAI Configuration")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(theme.colors.text)

            SettingsRow(label: "API Key") {
                BobeSecureField(
                    placeholder: openaiApiKey.isEmpty ? "sk-..." : "••••••••",
                    text: $openaiApiKey,
                    width: 280
                )
                    .onChange(of: openaiApiKey) { _, _ in isDirty = true }
            }
            SettingsRow(label: "Model") {
                BobeMenuPicker(
                    selection: $openaiModel,
                    options: openAIModelChoices,
                    label: { $0 },
                    width: 220
                )
                    .onChange(of: openaiModel) { _, _ in isDirty = true }
            }
        }
    }

    private var azureSettings: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Azure OpenAI Configuration")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(theme.colors.text)

            SettingsRow(label: "Endpoint") {
                BobeTextField(placeholder: "https://...", text: $azureEndpoint, width: 280)
                    .onChange(of: azureEndpoint) { _, _ in isDirty = true }
            }
            SettingsRow(label: "API Key") {
                BobeSecureField(
                    placeholder: azureApiKey.isEmpty ? "key" : "••••••••",
                    text: $azureApiKey,
                    width: 280
                )
                    .onChange(of: azureApiKey) { _, _ in isDirty = true }
            }
            SettingsRow(label: "Deployment") {
                BobeTextField(placeholder: "deployment-name", text: $azureDeployment, width: 200)
                    .onChange(of: azureDeployment) { _, _ in isDirty = true }
            }
        }
    }

    // MARK: - Actions

    private func loadSettings() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let s = try await DaemonClient.shared.getSettings()
            settings = s
            selectedProvider = ["ollama", "openai", "azure_openai"].contains(s.llmBackend) ? s.llmBackend : "ollama"
            ollamaModel = s.ollamaModel
            openaiModel = s.openaiModel
            azureEndpoint = s.azureOpenaiEndpoint
            azureDeployment = s.azureOpenaiDeployment

            let modelsResp = try await DaemonClient.shared.listModels()
            models = modelsResp.models
            isDirty = false
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func saveSettings() {
        isSaving = true
        Task {
            defer { isSaving = false }
            do {
                var req = SettingsUpdateRequest()
                req.llmBackend = selectedProvider
                req.ollamaModel = ollamaModel
                req.openaiModel = openaiModel
                if !openaiApiKey.isEmpty { req.openaiApiKey = openaiApiKey }
                req.azureOpenaiEndpoint = azureEndpoint
                req.azureOpenaiDeployment = azureDeployment
                if !azureApiKey.isEmpty { req.azureOpenaiApiKey = azureApiKey }
                _ = try await DaemonClient.shared.updateSettings(req)
                isDirty = false
                error = nil
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func discardChanges() {
        guard let s = settings else { return }
        selectedProvider = ["ollama", "openai", "azure_openai"].contains(s.llmBackend) ? s.llmBackend : "ollama"
        ollamaModel = s.ollamaModel
        openaiModel = s.openaiModel
        azureEndpoint = s.azureOpenaiEndpoint
        azureDeployment = s.azureOpenaiDeployment
        openaiApiKey = ""
        azureApiKey = ""
        isDirty = false
    }

    private func pullModel() async {
        isPulling = true
        defer { isPulling = false }
        do {
            try await DaemonClient.shared.pullModel(pullModelName)
            pullModelName = ""
            let resp = try await DaemonClient.shared.listModels()
            models = resp.models
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func deleteModel(_ name: String) async {
        do {
            try await DaemonClient.shared.deleteModel(name)
            let resp = try await DaemonClient.shared.listModels()
            models = resp.models
        } catch {
            self.error = error.localizedDescription
        }
    }
}

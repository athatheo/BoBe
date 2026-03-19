import SwiftUI

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
    @State private var ollamaUnavailableReason: String?
    @State private var visionModel = ""
    @State private var pullModelName = ""
    @State private var isPulling = false
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var isDirty = false
    @State private var error: String?
    @State private var popularModels: [String] = []
    @Environment(\.theme) private var theme

    private static let embeddingPatterns = ["embed", "bge", "minilm", "nomic-embed"]
    private static let visionPatterns = ["-vl", "-vision", "llava", "bakllava", "minicpm-v"]

    private var textModelOptions: [String] {
        self.models.map(\.name).filter { name in
            let lower = name.lowercased()
            return !Self.embeddingPatterns.contains(where: { lower.contains($0) })
                && !Self.visionPatterns.contains(where: { lower.contains($0) })
        }
    }

    private var visionModelOptions: [String] {
        self.models.map(\.name).filter { name in
            let lower = name.lowercased()
            return Self.visionPatterns.contains(where: { lower.contains($0) })
        }
    }

    private func modelRole(_ name: String) -> ModelRole {
        if name == self.ollamaModel { return .active }
        if name == self.visionModel { return .vision }
        let lower = name.lowercased()
        if Self.embeddingPatterns.contains(where: { lower.contains($0) }) { return .embedding }
        return .none
    }

    private enum ModelRole {
        case active, vision, embedding, none
    }

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
                            Text(L10n.tr("settings.ai_model.loading"))
                                .font(.system(size: 13))
                                .foregroundStyle(self.theme.colors.textMuted)
                        }
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.top, 40)
                    } else if self.settings != nil {
                        SettingsRow(label: L10n.tr("settings.ai_model.provider")) {
                            BobeMenuPicker(
                                selection: self.$selectedProvider,
                                options: ["ollama", "openai", "azure_openai"],
                                label: self.providerLabel(for:),
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
                    Text(L10n.tr("settings.ai_model.unsaved_changes"))
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    Spacer()
                    Button(L10n.tr("settings.shared.action.discard")) { self.discardChanges() }
                        .bobeButton(.secondary, size: .small)
                    Button(
                        self.isSaving
                            ? L10n.tr("settings.shared.action.saving")
                            : L10n.tr("settings.shared.action.save")
                    ) { self.saveSettings() }
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
                .animation(OverlayMotionRuntime.reduceMotion ? nil : .easeOut(duration: 0.2), value: self.isDirty)
            }
        }
        .task { await self.loadSettings() }
    }

    private var ollamaSettings: some View {
        VStack(alignment: .leading, spacing: 16) {
            CollapsibleSection(
                title: L10n.tr("settings.ai_model.ollama.active_model.title"),
                icon: "cpu",
                description: L10n.tr("settings.ai_model.ollama.active_model.description")
            ) {
                if self.models.isEmpty {
                    if let reason = self.ollamaUnavailableReason {
                        HStack(spacing: 6) {
                            Image(systemName: "exclamationmark.circle")
                                .foregroundStyle(self.theme.colors.primary)
                            Text(L10n.tr("settings.ai_model.ollama.unavailable_format", reason))
                                .font(.system(size: 12))
                                .foregroundStyle(self.theme.colors.primary)
                        }
                    } else {
                        Text(L10n.tr("settings.ai_model.ollama.no_models"))
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                } else {
                    BobeMenuPicker(
                        selection: self.$ollamaModel,
                        options: self.textModelOptions,
                        label: { $0 },
                        width: 260
                    )
                    .onChange(of: self.ollamaModel) { _, _ in self.isDirty = true }
                }
            }

            if !self.visionModelOptions.isEmpty {
                CollapsibleSection(
                    title: L10n.tr("settings.ai_model.ollama.capture_model.title"),
                    icon: "eye",
                    description: L10n.tr("settings.ai_model.ollama.capture_model.description")
                ) {
                    BobeMenuPicker(
                        selection: self.$visionModel,
                        options: self.visionModelOptions,
                        label: { $0 },
                        width: 260
                    )
                    .onChange(of: self.visionModel) { _, _ in self.isDirty = true }
                }
            }

            if !self.models.isEmpty {
                CollapsibleSection(
                    title: L10n.tr("settings.ai_model.ollama.installed_models.title"),
                    icon: "tray.full.fill",
                    description: L10n.tr("settings.ai_model.ollama.installed_models.description_format", self.models.count)
                ) {
                    ForEach(self.models) { model in
                        HStack(spacing: 8) {
                            Text(model.name)
                                .font(.system(size: 13, weight: .medium))

                            switch self.modelRole(model.name) {
                            case .active:
                                self.roleBadge(L10n.tr("settings.ai_model.ollama.active_badge"), color: self.theme.colors.secondary)
                            case .vision:
                                self.roleBadge(L10n.tr("settings.ai_model.ollama.vision_badge"), color: self.theme.colors.tertiary)
                            case .embedding:
                                self.roleBadge(L10n.tr("settings.ai_model.ollama.embedding_badge"), color: self.theme.colors.textMuted)
                            case .none:
                                EmptyView()
                            }

                            Spacer()

                            Text(formatBytes(model.sizeBytes))
                                .font(.system(size: 11))
                                .foregroundStyle(self.theme.colors.textMuted)

                            if self.modelRole(model.name) == .none {
                                Button(L10n.tr("settings.ai_model.ollama.action.use")) {
                                    self.ollamaModel = model.name
                                    self.isDirty = true
                                }
                                .bobeButton(.secondary, size: .mini)
                            }

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
                title: L10n.tr("settings.ai_model.ollama.download.title"),
                icon: "arrow.down.circle.fill",
                description: L10n.tr("settings.ai_model.ollama.download.description")
            ) {
                if self.isPulling {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text(L10n.tr("settings.ai_model.ollama.downloading_format", self.pullModelName))
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                } else {
                    HStack(spacing: 8) {
                        BobeTextField(placeholder: L10n.tr("settings.ai_model.ollama.model_name.placeholder"), text: self.$pullModelName) {
                            if !self.pullModelName.isEmpty { Task { await self.pullModel() } }
                        }
                        Button(L10n.tr("settings.ai_model.ollama.action.pull")) {
                            Task { await self.pullModel() }
                        }
                        .bobeButton(.secondary, size: .small)
                        .disabled(self.pullModelName.isEmpty)
                    }

                    if !self.popularModels.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(L10n.tr("settings.ai_model.ollama.popular_models"))
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(self.theme.colors.textMuted)

                            FlowLayout(spacing: 6) {
                                ForEach(self.popularModels, id: \.self) { name in
                                    Button(name) {
                                        self.pullModelName = name
                                    }
                                    .bobeButton(.secondary, size: .mini)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    private var openaiSettings: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(L10n.tr("settings.ai_model.openai.title"))
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(self.theme.colors.text)

            SettingsRow(label: L10n.tr("settings.ai_model.openai.api_key")) {
                BobeSecureField(
                    placeholder: self.openaiApiKey.isEmpty
                        ? L10n.tr("settings.ai_model.openai.api_key.placeholder")
                        : L10n.tr("settings.ai_model.secret.masked_placeholder"),
                    text: self.$openaiApiKey,
                    width: 280
                )
                .onChange(of: self.openaiApiKey) { _, _ in self.isDirty = true }
            }
            SettingsRow(label: L10n.tr("settings.ai_model.openai.model")) {
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
            Text(L10n.tr("settings.ai_model.azure.title"))
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(self.theme.colors.text)

            SettingsRow(label: L10n.tr("settings.ai_model.azure.endpoint")) {
                BobeTextField(placeholder: L10n.tr("settings.ai_model.azure.endpoint.placeholder"), text: self.$azureEndpoint, width: 280)
                    .onChange(of: self.azureEndpoint) { _, _ in self.isDirty = true }
            }
            SettingsRow(label: L10n.tr("settings.ai_model.azure.api_key")) {
                BobeSecureField(
                    placeholder: self.azureApiKey.isEmpty
                        ? L10n.tr("settings.ai_model.azure.api_key.placeholder")
                        : L10n.tr("settings.ai_model.secret.masked_placeholder"),
                    text: self.$azureApiKey,
                    width: 280
                )
                .onChange(of: self.azureApiKey) { _, _ in self.isDirty = true }
            }
            SettingsRow(label: L10n.tr("settings.ai_model.azure.deployment")) {
                BobeTextField(placeholder: L10n.tr("settings.ai_model.azure.deployment.placeholder"), text: self.$azureDeployment, width: 200)
                    .onChange(of: self.azureDeployment) { _, _ in self.isDirty = true }
            }
        }
    }

    private func loadSettings() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            let s = try await DaemonClient.shared.getSettings()
            self.settings = s
            self.selectedProvider = ["ollama", "openai", "azure_openai"].contains(s.llmBackend) ? s.llmBackend : "ollama"
            self.ollamaModel = s.ollamaModel
            self.visionModel = s.visionOllamaModel
            self.openaiModel = s.openaiModel
            self.azureEndpoint = s.azureOpenaiEndpoint
            self.azureDeployment = s.azureOpenaiDeployment

            self.onboardingOptions = try? await DaemonClient.shared.getOnboardingOptions()
            let modelsResp = try await DaemonClient.shared.listModels()
            self.models = modelsResp.models
            self.ollamaUnavailableReason = modelsResp.ollamaError
            self.isDirty = false

            Task { await self.fetchPopularModels() }
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
                req.visionOllamaModel = self.visionModel
                req.openaiModel = self.openaiModel
                if !self.openaiApiKey.isEmpty { req.openaiApiKey = self.openaiApiKey }
                req.azureOpenaiEndpoint = self.azureEndpoint
                req.azureOpenaiDeployment = self.azureDeployment
                if !self.azureApiKey.isEmpty { req.azureOpenaiApiKey = self.azureApiKey }
                _ = try await DaemonClient.shared.updateSettings(req)
                self.openaiApiKey = ""
                self.azureApiKey = ""
                self.error = nil
                await self.loadSettings()
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func discardChanges() {
        guard let s = settings else { return }
        self.selectedProvider = ["ollama", "openai", "azure_openai"].contains(s.llmBackend) ? s.llmBackend : "ollama"
        self.ollamaModel = s.ollamaModel
        self.visionModel = s.visionOllamaModel
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

    private func fetchPopularModels() async {
        do {
            guard let url = URL(string: "https://ollama.com/api/tags") else { return }
            var request = URLRequest(url: url)
            request.timeoutInterval = 10
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let httpResponse = response as? HTTPURLResponse,
                  (200 ... 299).contains(httpResponse.statusCode)
            else { return }

            struct OllamaTag: Decodable {
                let name: String
                let size: Int?

                enum CodingKeys: String, CodingKey {
                    case name, size
                }
            }
            struct OllamaTagsResponse: Decodable {
                let models: [OllamaTag]
            }

            let decoded = try JSONDecoder().decode(OllamaTagsResponse.self, from: data)
            let maxSize: Int = 20_000_000_000
            let installedNames = Set(self.models.map(\.name))
            let filtered = decoded.models
                .filter { tag in
                    let lower = tag.name.lowercased()
                    let withinSize = tag.size.map { $0 < maxSize } ?? true
                    let notEmbed = !Self.embeddingPatterns.contains(where: { lower.contains($0) })
                    let notVision = !Self.visionPatterns.contains(where: { lower.contains($0) })
                    let notInstalled = !installedNames.contains(tag.name)
                    return withinSize && notEmbed && notVision && notInstalled
                }
                .prefix(12)
                .map(\.name)
            self.popularModels = Array(filtered)
        } catch {
            // Silently ignore — suggestions are optional
        }
    }

    private func roleBadge(_ label: String, color: Color) -> some View {
        Text(label)
            .font(.system(size: 9, weight: .bold))
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Capsule().fill(color.opacity(0.15)))
    }

    private func providerLabel(for provider: String) -> String {
        switch provider {
        case "ollama": L10n.tr("settings.ai_model.provider.ollama")
        case "openai": L10n.tr("settings.ai_model.provider.openai")
        case "azure_openai": L10n.tr("settings.ai_model.provider.azure_openai")
        default: provider
        }
    }
}

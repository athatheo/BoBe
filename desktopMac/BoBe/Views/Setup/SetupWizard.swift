import AppKit
import SwiftUI

struct SetupWizard: View {
    // @State vars are internal (not private) because they are accessed
    // from extension files (SetupWizardActions, ConfigViews, ProgressViews).
    @State var step: SetupStep = .welcome
    @State var setupMode: SetupMode = .local
    @State var selectedLocalModel = "small"
    @State var selectedOpenAIModel = defaultOpenAIModelOption.modelName
    @State var apiKey = ""
    @State var onlineProvider = "azure_openai"
    @State var onlineModel = onlineProviders.first { $0.id == "azure_openai" }?.defaultModel ?? ""
    @State var endpoint = ""
    @State var showAzure = false
    @State var progressPercent: Double = 0
    @State var progressMessage = ""
    @State var errorMessage = ""
    @State var busy = false
    @State var isFinishingSetup = false
    @State var captureSkipped = false
    @State var visionDownloaded = false
    @State var visionProgress: Double = 0
    @State var visionMessage = ""
    @State var visionDownloading = false
    @State var visionError = ""
    @State var screenPermission = "not-determined"
    @State var permissionPollTask: Task<Void, Never>?
    @Environment(\.theme) var theme

    var selectedModelOption: ModelOption {
        localModelOptions.first { $0.id == selectedLocalModel } ?? defaultModelOption
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                if let logoURL = Bundle.main.url(forResource: "logo-128", withExtension: "png"),
                   let nsImage = NSImage(contentsOf: logoURL) {
                    Image(nsImage: nsImage)
                        .resizable()
                        .frame(width: 28, height: 28)
                } else {
                    Text("BoBe Setup")
                        .font(.system(size: 18, weight: .bold))
                        .foregroundStyle(theme.colors.primary)
                }
                Text("BoBe")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundStyle(theme.colors.primary)
            }
            .padding(.top, 28)
            .padding(.bottom, 20)

            Group {
                switch step {
                case .welcome: welcomeView
                case .chooseMode: chooseModeView
                case .cloudConfig: cloudConfigView
                case .localConfig: localConfigView
                case .downloadingEngine, .downloadingModel, .initializing: downloadingView
                case .captureSetup: captureSetupView
                case .complete: completeView
                case .error: errorView
                }
            }
            .padding(.horizontal, 36)

            Spacer()
        }
        .frame(width: 540, height: 700)
        .background(theme.colors.background)
        .onDisappear { permissionPollTask?.cancel() }
        .onChange(of: step) { _, newStep in
            permissionPollTask?.cancel()
            if newStep == .captureSetup {
                checkScreenPermission()
                startPermissionPolling()
                autoStartVisionDownload()
            }
        }
    }

    // MARK: - Welcome

    private var welcomeView: some View {
        VStack(spacing: 0) {
            if let logoURL = Bundle.main.url(forResource: "logo-128", withExtension: "png"),
               let nsImage = NSImage(contentsOf: logoURL) {
                Image(nsImage: nsImage)
                    .resizable()
                    .frame(width: 72, height: 72)
                    .padding(.bottom, 12)
            }

            Text("Welcome to BoBe")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)
                .padding(.bottom, 4)

            Text("Your proactive AI companion for Mac")
                .font(.system(size: 15))
                .foregroundStyle(theme.colors.textMuted)
                .padding(.bottom, 24)

            VStack(spacing: 12) {
                ConceptCard(
                    icon: "person.fill",
                    title: "BoBe Souls",
                    description: "BoBe's personality that shapes how it communicates. Warm, focused, and respectful of your workflow."
                )
                ConceptCard(
                    icon: "target",
                    title: "Goals",
                    description: "BoBe tracks what you're working toward and helps you make progress — automatically or when you ask."
                )
                ConceptCard(
                    icon: "brain.head.profile",
                    title: "Memories",
                    description: "BoBe remembers your preferences, projects, and context so every conversation builds on the last."
                )
            }
            .padding(.bottom, 24)

            Button("Get Started") { step = .chooseMode }
                .bobeButton(.primary, size: .regular)
        }
        .frame(maxWidth: 440)
    }

    // MARK: - AI Choice

    private var chooseModeView: some View {
        VStack(spacing: 0) {
            Text("Choose your AI")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)
                .padding(.bottom, 4)

            Text("BoBe needs an AI model to think with.")
                .font(.system(size: 15))
                .foregroundStyle(theme.colors.textMuted)
                .padding(.bottom, 24)

            HStack(spacing: 16) {
                AIChoiceCard(
                    icon: "cloud.fill",
                    title: "Cloud Models",
                    subtitle: "Recommended — fastest setup"
                ) {
                    step = .cloudConfig
                }

                AIChoiceCard(
                    icon: "desktopcomputer",
                    title: "Local Models",
                    subtitle: "Runs entirely on your Mac"
                ) {
                    step = .localConfig
                }
            }
        }
        .frame(maxWidth: 440)
    }

    // MARK: - Error

    private var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 40)).foregroundStyle(theme.colors.primary)
            Text("Setup failed")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(theme.colors.text)
            Text(errorMessage)
                .font(.system(size: 14)).foregroundStyle(theme.colors.textMuted)
                .multilineTextAlignment(.center)
            Button("Retry") {
                step = .chooseMode
                errorMessage = ""
                progressPercent = 0
                progressMessage = ""
            }
            .bobeButton(.primary, size: .regular)
        }
        .frame(maxWidth: 420)
    }
}

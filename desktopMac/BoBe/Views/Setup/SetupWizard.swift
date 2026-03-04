import AppKit
import SwiftUI

struct SetupWizard: View {
    @State var step: SetupStep = .welcome
    @State var setupMode: SetupMode = .local
    @State var options: OnboardingOptions?
    @State var selectedTier = "small"
    @State var selectedProvider = "openai"
    @State var selectedModel = ""
    @State var apiKey = ""
    @State var endpoint = ""
    @State var deployment = ""
    @State var setupJob: SetupJobState?
    @State var pollTask: Task<Void, Never>?
    @State var progressPercent: Double = 0
    @State var progressMessage = ""
    @State var errorMessage = ""
    @State var busy = false
    @State var isFinishingSetup = false
    @State var screenPermission: ScreenPermissionStatus = .notDetermined
    @State var hasRequestedCaptureAccess = false
    @State var permissionPollTask: Task<Void, Never>?
    @Environment(\.theme) var theme

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
                        .foregroundStyle(self.theme.colors.primary)
                }
                Text("BoBe")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundStyle(self.theme.colors.primary)
            }
            .padding(.top, 28)
            .padding(.bottom, 20)

            Group {
                switch self.step {
                case .welcome: self.welcomeView
                case .chooseMode: self.chooseModeView
                case .cloudConfig: cloudConfigView
                case .localConfig: localConfigView
                case .setupInProgress: setupProgressView
                case .captureSetup: captureSetupView
                case .complete: completeView
                case .error: self.errorView
                }
            }
            .padding(.horizontal, 36)

            Spacer()
        }
        .frame(width: 540, height: 700)
        .background(self.theme.colors.background)
        .onDisappear {
            self.permissionPollTask?.cancel()
            self.pollTask?.cancel()
        }
        .onChange(of: self.step) { _, newStep in
            self.permissionPollTask?.cancel()
            if newStep == .captureSetup {
                checkScreenPermission()
                startPermissionPolling()
            }
        }
        .task { await self.loadOptions() }
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
                .foregroundStyle(self.theme.colors.text)
                .padding(.bottom, 4)

            Text("Your proactive AI companion for Mac")
                .font(.system(size: 15))
                .foregroundStyle(self.theme.colors.textMuted)
                .padding(.bottom, 24)

            VStack(spacing: 12) {
                ConceptCard(
                    icon: "person.fill", title: "BoBe Souls",
                    description: "BoBe's personality that shapes how it communicates."
                )
                ConceptCard(
                    icon: "target", title: "Goals",
                    description: "BoBe tracks what you're working toward and helps you make progress."
                )
                ConceptCard(
                    icon: "brain.head.profile", title: "Memories",
                    description: "BoBe remembers your preferences, projects, and context."
                )
            }
            .padding(.bottom, 24)

            Button("Get Started") { self.step = .chooseMode }
                .bobeButton(.primary, size: .regular)
                .disabled(self.options == nil)

            if self.options == nil {
                ProgressView()
                    .scaleEffect(0.7)
                    .padding(.top, 8)
            }
        }
        .frame(maxWidth: 440)
    }

    // MARK: - Choose Mode

    private var chooseModeView: some View {
        VStack(spacing: 0) {
            Text("Choose your AI")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(self.theme.colors.text)
                .padding(.bottom, 4)

            Text("BoBe needs an AI model to think with.")
                .font(.system(size: 15))
                .foregroundStyle(self.theme.colors.textMuted)
                .padding(.bottom, 24)

            HStack(spacing: 16) {
                AIChoiceCard(icon: "cloud.fill", title: "Cloud Models", subtitle: "Recommended — fastest setup") {
                    self.setupMode = .online
                    self.step = .cloudConfig
                }
                AIChoiceCard(icon: "desktopcomputer", title: "Local Models", subtitle: "Runs entirely on your Mac") {
                    self.setupMode = .local
                    self.step = .localConfig
                }
            }
        }
        .frame(maxWidth: 440)
    }

    // MARK: - Error

    private var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 40)).foregroundStyle(self.theme.colors.primary)
            Text("Setup failed")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(self.theme.colors.text)
            Text(self.errorMessage)
                .font(.system(size: 14)).foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
            Button("Retry") {
                self.step = .chooseMode
                self.errorMessage = ""
                self.progressPercent = 0
                self.progressMessage = ""
            }
            .bobeButton(.primary, size: .regular)
        }
        .frame(maxWidth: 420)
    }

    func loadOptions() async {
        for attempt in 1 ... 5 {
            guard !Task.isCancelled else { return }
            do {
                self.options = try await DaemonClient.shared.getOnboardingOptions()
                if let firstModel = options?.cloudProviders.first?.models.first?.id {
                    self.selectedModel = firstModel
                }
                return
            } catch {
                if attempt < 5 {
                    try? await Task.sleep(for: .seconds(Double(attempt)))
                }
            }
        }
        self.errorMessage = "Could not connect to the backend. Please restart BoBe."
        self.step = .error
    }
}

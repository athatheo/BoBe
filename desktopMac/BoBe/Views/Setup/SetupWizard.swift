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
                    Text(L10n.tr("setup.window.title"))
                        .font(.system(size: 18, weight: .bold))
                        .foregroundStyle(self.theme.colors.primary)
                }
                Text(L10n.tr("app.brand_name"))
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

    private var welcomeView: some View {
        VStack(spacing: 0) {
            if let logoURL = Bundle.main.url(forResource: "logo-128", withExtension: "png"),
               let nsImage = NSImage(contentsOf: logoURL) {
                Image(nsImage: nsImage)
                    .resizable()
                    .frame(width: 72, height: 72)
                    .padding(.bottom, 12)
            }

            Text(L10n.tr("setup.welcome.title"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(self.theme.colors.text)
                .padding(.bottom, 4)

            Text(L10n.tr("setup.welcome.subtitle"))
                .font(.system(size: 15))
                .foregroundStyle(self.theme.colors.textMuted)
                .padding(.bottom, 24)

            VStack(spacing: 12) {
                ConceptCard(
                    icon: "person.fill", title: L10n.tr("setup.welcome.concept.souls.title"),
                    description: L10n.tr("setup.welcome.concept.souls.description")
                )
                ConceptCard(
                    icon: "target", title: L10n.tr("setup.welcome.concept.goals.title"),
                    description: L10n.tr("setup.welcome.concept.goals.description")
                )
                ConceptCard(
                    icon: "brain.head.profile", title: L10n.tr("setup.welcome.concept.memories.title"),
                    description: L10n.tr("setup.welcome.concept.memories.description")
                )
            }
            .padding(.bottom, 24)

            Button(L10n.tr("setup.welcome.get_started")) { self.step = .chooseMode }
                .bobeButton(.primary, size: .regular)
                .disabled(self.options == nil)

            if self.options == nil {
                ProgressView()
                    .tint(self.theme.colors.primary)
                    .scaleEffect(0.7)
                    .padding(.top, 8)
            }
        }
        .frame(maxWidth: 440)
    }

    private var chooseModeView: some View {
        VStack(spacing: 0) {
            Text(L10n.tr("setup.choose_mode.title"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(self.theme.colors.text)
                .padding(.bottom, 4)

            Text(L10n.tr("setup.choose_mode.subtitle"))
                .font(.system(size: 15))
                .foregroundStyle(self.theme.colors.textMuted)
                .padding(.bottom, 24)

            HStack(spacing: 16) {
                AIChoiceCard(
                    icon: "cloud.fill",
                    title: L10n.tr("setup.choose_mode.cloud.title"),
                    subtitle: L10n.tr("setup.choose_mode.cloud.subtitle")
                ) {
                    self.setupMode = .online
                    self.step = .cloudConfig
                }
                AIChoiceCard(
                    icon: "desktopcomputer",
                    title: L10n.tr("setup.choose_mode.local.title"),
                    subtitle: L10n.tr("setup.choose_mode.local.subtitle")
                ) {
                    self.setupMode = .local
                    self.step = .localConfig
                }
            }
        }
        .frame(maxWidth: 440)
    }

    private var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 40)).foregroundStyle(self.theme.colors.primary)
            Text(L10n.tr("setup.error.title"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(self.theme.colors.text)
            Text(self.errorMessage)
                .font(.system(size: 14)).foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
            Button(L10n.tr("setup.error.retry")) {
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
        self.errorMessage = L10n.tr("setup.error.backend_unreachable")
        self.step = .error
    }
}

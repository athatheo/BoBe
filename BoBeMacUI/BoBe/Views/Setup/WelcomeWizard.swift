import AppKit
import SwiftUI

enum WelcomeStep: Hashable {
    case welcome
    case engineChoice
    case cloudAuth
    case localSetup
    case personalize
    case proactivity
    case permissions
    case voiceSetup
    case done

    /// Cloud/local share one position because they are two branches of the
    /// same intelligence setup decision.
    var progressOrdinal: Int {
        switch self {
        case .welcome: 0
        case .engineChoice: 1
        case .cloudAuth, .localSetup: 2
        case .personalize: 3
        case .proactivity: 4
        case .permissions: 5
        case .voiceSetup: 6
        case .done: 7
        }
    }
}

enum EngineChoice: String, Sendable {
    case copilot
    case local
}

struct WelcomeWizard: View {
    let onComplete: () -> Void
    /// Optional deep-link entry point. When set (e.g. from a Finish-Setup
    /// pill on the overlay), the wizard skips straight to the step that
    /// still needs the user's attention rather than restarting from scratch.
    var initialStep: WelcomeStep?

    @State private var currentStep: WelcomeStep
    @State private var engineChoice: EngineChoice?
    @State private var preferredName = ""
    @State private var firstGoal = ""
    @State private var proactivityLevel: ProactivityLevel = .balanced
    @State private var history: [WelcomeStep] = []
    @State private var isLeavingLocalSetup = false
    @Environment(\.theme) private var theme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let themeStore = ThemeStore.shared
    private let totalSteps = 8

    init(onComplete: @escaping () -> Void, initialStep: WelcomeStep? = nil) {
        self.onComplete = onComplete
        self.initialStep = initialStep
        self._currentStep = State(initialValue: initialStep ?? .welcome)
    }

    var body: some View {
        VStack(spacing: 0) {
            self.progressBar
                .padding(.top, 20)
                .padding(.horizontal, 32)

            self.stepContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding(.horizontal, 40)
                .padding(.vertical, 24)
                .transition(self.stepTransition)
                .id(self.currentStep)
        }
        .frame(width: 540, height: 640)
        .background(self.themeStore.currentTheme.colors.background)
        .environment(\.theme, self.themeStore.currentTheme)
        .preferredColorScheme(self.themeStore.preferredColorScheme)
        .onChange(of: self.reduceMotion, initial: true) { _, value in
            OverlayMotionRuntime.reduceMotion = value
        }
        .confirmationDialog(
            L10n.tr("setup.local.leave.title"),
            isPresented: self.$isLeavingLocalSetup,
            titleVisibility: .visible
        ) {
            Button(L10n.tr("setup.local.leave.keep")) { self.finishGoingBack(cancelInstall: false) }
            Button(L10n.tr("setup.local.leave.cancel"), role: .destructive) {
                self.finishGoingBack(cancelInstall: true)
            }
            Button(L10n.tr("settings.editor.action.cancel"), role: .cancel) {}
        } message: {
            Text(L10n.tr("setup.local.leave.message"))
        }
    }

    @ViewBuilder
    private var stepContent: some View {
        switch self.currentStep {
        case .welcome:
            WelcomeValueStepView(onContinue: { self.advance() })
        case .engineChoice:
            EngineChoiceStepView(selection: self.$engineChoice) { self.advance() }
        case .cloudAuth:
            CloudAuthStepView(onContinue: { self.advance() })
        case .localSetup:
            LocalSetupStepView(onContinue: { self.advance() })
        case .personalize:
            PersonalizeStepView(
                preferredName: self.$preferredName,
                firstGoal: self.$firstGoal,
                onContinue: { self.advance() }
            )
        case .proactivity:
            ProactivityStepView(selection: self.$proactivityLevel, onContinue: { self.advance() })
        case .permissions:
            PermissionsStepView(
                screenContextEnabled: self.proactivityLevel.settings.captureEnabled,
                onContinue: { self.advance() }
            )
        case .voiceSetup:
            VoiceSetupStepView(onContinue: { self.advance() })
        case .done:
            DoneStepView(
                engineChoice: self.engineChoice,
                preferredName: self.preferredName,
                firstGoal: self.firstGoal,
                proactivityLevel: self.proactivityLevel,
                onLaunch: self.onComplete
            )
        }
    }

    /// Subtle Back affordance integrated into the progress row.
    /// Renders inline (no extra row) so progress dots stay visually centred.
    private var progressBar: some View {
        ZStack(alignment: .leading) {
            HStack(spacing: 6) {
                ForEach(0 ..< self.totalSteps, id: \.self) { idx in
                    Capsule()
                        .fill(idx <= self.currentStep.progressOrdinal
                            ? self.theme.colors.primary
                            : self.theme.colors.border.opacity(0.6))
                        .frame(height: 4)
                }
            }

            if self.canGoBack {
                Button(action: self.goBack) {
                    HStack(spacing: 4) {
                        Image(systemName: "chevron.left")
                            .font(.system(size: 11, weight: .semibold))
                        Text(L10n.tr("setup.wizard.back"))
                            .bobeTextStyle(.helper)
                            .fontWeight(.medium)
                    }
                    .foregroundStyle(self.theme.colors.textMuted)
                    .padding(.vertical, 4)
                    .padding(.horizontal, 8)
                    .background(
                        Capsule().fill(self.theme.colors.surface.opacity(0.6))
                    )
                    .offset(y: 18)
                }
                .buttonStyle(.plain)
                .keyboardShortcut("[", modifiers: .command)
                .accessibilityLabel(L10n.tr("setup.wizard.back.accessibility"))
                .accessibilityIdentifier("setup.wizard.back")
                .transition(.opacity)
            }
        }
    }

    private var canGoBack: Bool {
        self.currentStep != .done && !self.history.isEmpty
    }

    private var stepTransition: AnyTransition {
        if OverlayMotionRuntime.reduceMotion {
            return .opacity
        }
        return .asymmetric(
            insertion: .move(edge: .trailing).combined(with: .opacity),
            removal: .move(edge: .leading).combined(with: .opacity)
        )
    }

    private func advance() {
        let next: WelcomeStep
        switch self.currentStep {
        case .welcome:
            next = .engineChoice
        case .engineChoice:
            next = self.engineChoice == .local ? .localSetup : .cloudAuth
        case .cloudAuth, .localSetup:
            next = .personalize
        case .personalize:
            next = .proactivity
        case .proactivity:
            next = .permissions
        case .permissions:
            next = .voiceSetup
        case .voiceSetup:
            if self.initialStep == .voiceSetup {
                self.onComplete()
                return
            }
            next = .done
        case .done:
            return
        }
        self.history.append(self.currentStep)
        withAnimation(OverlayMotionRuntime.reduceMotion ? nil : .easeOut(duration: 0.24)) {
            self.currentStep = next
        }
    }

    private func goBack() {
        if self.currentStep == .localSetup {
            self.isLeavingLocalSetup = true
            return
        }
        self.finishGoingBack(cancelInstall: false)
    }

    private func finishGoingBack(cancelInstall: Bool) {
        guard let previous = self.history.popLast() else { return }
        if cancelInstall {
            Task { try? await DaemonClient.shared.cancelLocalRuntimeInstall() }
        }
        withAnimation(OverlayMotionRuntime.reduceMotion ? nil : .easeOut(duration: 0.24)) {
            self.currentStep = previous
        }
    }
}

import AppKit
import SwiftUI

enum WelcomeStep: Hashable {
    case engineChoice
    case cloudAuth
    case localSetup
    case permissions
    case voiceSetup
    case done

    /// 0..4 axis for progress dots; cloud/local share position 1.
    var progressOrdinal: Int {
        switch self {
        case .engineChoice: 0
        case .cloudAuth, .localSetup: 1
        case .permissions: 2
        case .voiceSetup: 3
        case .done: 4
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
    @State private var history: [WelcomeStep] = []
    @Environment(\.theme) private var theme
    private let themeStore = ThemeStore.shared
    private let totalSteps = 5

    init(onComplete: @escaping () -> Void, initialStep: WelcomeStep? = nil) {
        self.onComplete = onComplete
        self.initialStep = initialStep
        self._currentStep = State(initialValue: initialStep ?? .engineChoice)
    }

    var body: some View {
        VStack(spacing: 0) {
            self.progressBar
                .padding(.top, 20)
                .padding(.horizontal, 32)

            Group {
                switch self.currentStep {
                case .engineChoice:
                    EngineChoiceStepView(selection: self.$engineChoice) {
                        self.advance()
                    }
                case .cloudAuth:
                    CloudAuthStepView(onContinue: { self.advance() })
                case .localSetup:
                    LocalSetupStepView(onContinue: { self.advance() })
                case .permissions:
                    PermissionsStepView(onContinue: { self.advance() })
                case .voiceSetup:
                    VoiceSetupStepView(onContinue: { self.advance() })
                case .done:
                    DoneStepView(engineChoice: self.engineChoice, onLaunch: self.onComplete)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.horizontal, 40)
            .padding(.vertical, 24)
            .transition(self.stepTransition)
            .id(self.currentStep)
        }
        .frame(width: 540, height: 640)
        .background(self.themeStore.currentTheme.colors.background)
        .environment(\.theme, self.themeStore.currentTheme)
        .preferredColorScheme(self.themeStore.currentTheme.isDark ? .dark : .light)
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
                            .font(.system(size: 12, weight: .medium))
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
                .transition(.opacity)
            }
        }
    }

    private var canGoBack: Bool {
        !self.history.isEmpty
    }

    private var stepTransition: AnyTransition {
        if OverlayMotionRuntime.reduceMotion { return .opacity }
        return .asymmetric(
            insertion: .move(edge: .trailing).combined(with: .opacity),
            removal: .move(edge: .leading).combined(with: .opacity)
        )
    }

    private func advance() {
        let next: WelcomeStep
        switch self.currentStep {
        case .engineChoice:
            next = self.engineChoice == .local ? .localSetup : .cloudAuth
        case .cloudAuth, .localSetup:
            next = .permissions
        case .permissions:
            next = .voiceSetup
        case .voiceSetup:
            next = .done
        case .done:
            return
        }
        self.history.append(self.currentStep)
        withAnimation(.easeOut(duration: 0.32)) {
            self.currentStep = next
        }
    }

    private func goBack() {
        guard let previous = self.history.popLast() else { return }
        withAnimation(.easeOut(duration: 0.32)) {
            self.currentStep = previous
        }
    }
}

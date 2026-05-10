import AppKit
import SwiftUI

enum WelcomeStep: Hashable {
    case welcome
    case engineChoice
    case cloudAuth
    case localSetup
    case permissions
    case done

    /// 0..4 axis for progress dots; cloud/local share position 2.
    var progressOrdinal: Int {
        switch self {
        case .welcome: 0
        case .engineChoice: 1
        case .cloudAuth, .localSetup: 2
        case .permissions: 3
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

    @State private var currentStep: WelcomeStep = .welcome
    @State private var engineChoice: EngineChoice?
    @Environment(\.theme) private var theme
    private let themeStore = ThemeStore.shared
    private let totalSteps = 5

    var body: some View {
        VStack(spacing: 0) {
            self.progressIndicator
                .padding(.top, 24)
                .padding(.horizontal, 32)

            Group {
                switch self.currentStep {
                case .welcome:
                    WelcomeStepView(onContinue: { self.advance() })
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
                case .done:
                    DoneStepView(engineChoice: self.engineChoice, onLaunch: self.onComplete)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.horizontal, 40)
            .padding(.vertical, 24)
        }
        .frame(width: 540, height: 640)
        .background(self.themeStore.currentTheme.colors.background)
        .environment(\.theme, self.themeStore.currentTheme)
        .preferredColorScheme(self.themeStore.currentTheme.isDark ? .dark : .light)
    }

    private var progressIndicator: some View {
        HStack(spacing: 6) {
            ForEach(0..<self.totalSteps, id: \.self) { idx in
                Capsule()
                    .fill(idx <= self.currentStep.progressOrdinal
                        ? self.theme.colors.primary
                        : self.theme.colors.border.opacity(0.6))
                    .frame(height: 4)
            }
        }
    }

    private func advance() {
        let next: WelcomeStep
        switch self.currentStep {
        case .welcome:
            next = .engineChoice
        case .engineChoice:
            next = self.engineChoice == .local ? .localSetup : .cloudAuth
        case .cloudAuth, .localSetup:
            next = .permissions
        case .permissions:
            next = .done
        case .done:
            return
        }
        self.currentStep = next
    }
}

import AppKit
import SwiftUI

enum WelcomeStep: CaseIterable {
    case welcome
    case copilotCheck
    case permissions
    case done

    var index: Int {
        Self.allCases.firstIndex(of: self) ?? 0
    }
}

struct WelcomeWizard: View {
    let onComplete: () -> Void

    @State private var currentStep: WelcomeStep = .welcome
    @Environment(\.theme) private var theme
    private let themeStore = ThemeStore.shared

    var body: some View {
        VStack(spacing: 0) {
            self.progressIndicator
                .padding(.top, 24)
                .padding(.horizontal, 32)

            Group {
                switch self.currentStep {
                case .welcome:
                    WelcomeStepView(onContinue: { self.advance() })
                case .copilotCheck:
                    CopilotCheckStepView(onContinue: { self.advance() })
                case .permissions:
                    PermissionsStepView(onContinue: { self.advance() })
                case .done:
                    DoneStepView(onLaunch: self.onComplete)
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
            ForEach(WelcomeStep.allCases, id: \.self) { step in
                Capsule()
                    .fill(self.color(for: step))
                    .frame(height: 4)
            }
        }
    }

    private func color(for step: WelcomeStep) -> Color {
        if step.index <= self.currentStep.index {
            return self.theme.colors.primary
        }
        return self.theme.colors.border.opacity(0.6)
    }

    private func advance() {
        let allCases = WelcomeStep.allCases
        let currentIdx = self.currentStep.index
        let nextIdx = currentIdx + 1
        if nextIdx < allCases.count {
            self.currentStep = allCases[nextIdx]
        }
    }
}

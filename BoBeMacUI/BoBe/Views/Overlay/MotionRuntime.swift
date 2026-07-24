import AppKit
import SwiftUI

enum OverlayMotionPrimitive {
    case breathing
    case chatTransition
    case indicatorTransition
    case badgePulse
    case statusLabelTransition
}

enum OverlayMotionRuntime {
    static let chatTransitionSeconds: TimeInterval = 0.16

    @MainActor static var reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion

    @MainActor static var shouldAnimate: Bool {
        !reduceMotion
    }

    @MainActor
    static func animation(for primitive: OverlayMotionPrimitive) -> Animation? {
        guard self.shouldAnimate else { return nil }
        switch primitive {
        case .breathing:
            return .easeInOut(duration: 2.8)
        case .chatTransition:
            return .easeOut(duration: self.chatTransitionSeconds)
        case .indicatorTransition:
            return .easeOut(duration: 0.18)
        case .badgePulse:
            return .easeInOut(duration: 2.0)
        case .statusLabelTransition:
            return .easeInOut(duration: 0.2)
        }
    }

    @MainActor
    static func breathingScale(isExpanded: Bool) -> CGFloat {
        guard self.shouldAnimate else { return 1.0 }
        return isExpanded ? 1.012 : 0.994
    }
}

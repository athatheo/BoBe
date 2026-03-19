import SwiftUI

enum OverlayMotionPrimitive {
    case hover
    case breathing
    case chatTransition
    case indicatorTransition
    case badgePulse
    case statusLabelTransition
}

enum OverlayMotionRuntime {
    @MainActor static var reduceMotion = false

    @MainActor static var shouldAnimate: Bool {
        !reduceMotion
    }

    static func animation(for primitive: OverlayMotionPrimitive) -> Animation {
        switch primitive {
        case .hover:
            .spring(duration: 0.2, bounce: 0.15)
        case .breathing:
            .easeInOut(duration: 2.8)
        case .chatTransition, .indicatorTransition:
            .spring(duration: 0.32, bounce: 0.14)
        case .badgePulse:
            .easeInOut(duration: 2.0)
        case .statusLabelTransition:
            .easeInOut(duration: 0.2)
        }
    }

    @MainActor static func breathingScale(isExpanded: Bool) -> CGFloat {
        guard shouldAnimate else { return 1.0 }
        return isExpanded ? 1.012 : 0.994
    }

    @MainActor static func hoverScale(isHovered: Bool) -> CGFloat {
        guard shouldAnimate else { return 1.0 }
        return isHovered ? 1.06 : 1.0
    }

    @MainActor static func hoverYOffset(isHovered: Bool) -> CGFloat {
        guard shouldAnimate else { return 0 }
        return isHovered ? -1.0 : 0
    }
}

import SwiftUI

enum OverlayMotionPrimitive {
    case hover
    case breathing
    case chatTransition
    case indicatorTransition
    case badgePulse
    case statusLabelTransition
}

/// Centralized animation timing and scale values for all overlay motion.
/// Keeps animation behavior consistent across avatar, chat, and indicator transitions.
enum OverlayMotionRuntime {
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

    static func breathingScale(isExpanded: Bool) -> CGFloat {
        isExpanded ? 1.012 : 0.994
    }

    static func hoverScale(isHovered: Bool) -> CGFloat {
        isHovered ? 1.06 : 1.0
    }

    static func hoverYOffset(isHovered: Bool) -> CGFloat {
        isHovered ? -1.0 : 0
    }
}

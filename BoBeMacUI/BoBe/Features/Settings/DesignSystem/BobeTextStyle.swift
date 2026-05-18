import SwiftUI

/// Centralized typography palette. Every text-bearing surface in the app
/// reaches through `.bobeTextStyle(_:)` so font/size/weight changes are
/// one-place edits.
enum BobeTextStyle {
    case windowTitle
    case sectionLabel
    case rowTitle
    case rowMeta
    case helper
    case body
    case badge

    case overlayStatus
    case chatSender
    case chatPending
    case chatBody
    case chatMeta
    case brandLabel

    case setupTitle
    case setupSubtitle
    case setupBody
    case setupHeading

    case inputField
    case heading

    /// 13pt regular — settings-panel descriptions and secondary labels.
    /// Keeps `bobeTextStyle()` adoption consistent across the panels so
    /// future typography tweaks happen in one place.
    case settingsBody

    /// Styles in this palette anchor to Dynamic Type semantic ramps whenever
    /// the base size is ≥11pt — so System Settings → Accessibility → Display →
    /// Text Size scales them. The handful of sub-11pt styles below (badges,
    /// chat-sender labels, sectionLabel) stay on `.system(size:)` because
    /// `.subheadline` (11pt on macOS) is already bigger than what they need.
    var font: Font {
        switch self {
        // Scaling styles — Dynamic Type ramp
        case .windowTitle:
            .largeTitle.weight(.semibold)
        case .setupTitle:
            .largeTitle.bold()
        case .setupSubtitle:
            .title3
        case .rowTitle:
            .body.weight(.semibold)
        case .setupHeading:
            .body.weight(.semibold)
        case .heading:
            .body.weight(.semibold)
        case .inputField:
            .body
        case .settingsBody:
            .body
        case .setupBody:
            .body
        case .body:
            .callout
        case .chatBody:
            .callout
        case .rowMeta:
            .subheadline
        case .helper:
            .subheadline
        case .brandLabel:
            .subheadline.bold()
        // Static styles — below the smallest Dynamic Type rung (11pt on
        // macOS), or visually-fixed labels (sectionLabel small-caps).
        case .sectionLabel:
            .system(size: 10, weight: .semibold)
        case .overlayStatus:
            .system(size: 10)
        case .chatMeta:
            .system(size: 10, weight: .medium)
        case .badge:
            .system(size: 9, weight: .medium)
        case .chatSender:
            .system(size: 9, weight: .semibold)
        case .chatPending:
            .system(size: 8)
        }
    }
}

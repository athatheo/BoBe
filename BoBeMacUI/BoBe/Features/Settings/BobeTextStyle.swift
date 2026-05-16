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

    var font: Font {
        switch self {
        case .windowTitle:
            .system(size: 28, weight: .semibold)
        case .sectionLabel:
            .system(size: 10, weight: .semibold)
        case .rowTitle:
            .system(size: 13, weight: .semibold)
        case .rowMeta:
            .system(size: 11)
        case .helper:
            .system(size: 11)
        case .body:
            .system(size: 12)
        case .badge:
            .system(size: 9, weight: .medium)
        case .overlayStatus:
            .system(size: 10)
        case .chatSender:
            .system(size: 9, weight: .semibold)
        case .chatPending:
            .system(size: 8)
        case .chatBody:
            .system(size: 12)
        case .chatMeta:
            .system(size: 10, weight: .medium)
        case .brandLabel:
            .system(size: 11, weight: .bold)
        case .setupTitle:
            .system(size: 26, weight: .bold)
        case .setupSubtitle:
            .system(size: 15)
        case .setupBody:
            .system(size: 14)
        case .setupHeading:
            .system(size: 14, weight: .semibold)
        case .inputField:
            .system(size: 13)
        case .heading:
            .system(size: 14, weight: .semibold)
        }
    }
}

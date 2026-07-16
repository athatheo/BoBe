import Foundation
import Observation

/// Client-side toggle that gates power-user UI. Backed by UserDefaults
/// because nothing about it needs to round-trip through the daemon —
/// hiding/showing chrome is purely a UI concern, and keeping it out of
/// `DaemonSettings` means there's no PATCH round-trip on toggle.
///
/// Read sites:
/// - `VoicePanel` (model codenames, daemon URLs)
/// - `MCPServersPanel` (raw JSON editor)
/// - `EnginePanel` (strict offline, Sign in via Terminal fallback)
/// - context editors (guided versus raw Markdown)
///
/// Default: **off** (novice). Surfacing one knob is cheaper than asking
/// a new user to wade through fifteen.
@MainActor
@Observable
final class ExpertMode {
    static let shared = ExpertMode()

    private static let key = "bobe.expert_mode"

    private(set) var isEnabled: Bool

    private init() {
        self.isEnabled = UserDefaults.standard.bool(forKey: Self.key)
    }

    func setEnabled(_ enabled: Bool) {
        guard self.isEnabled != enabled else { return }
        self.isEnabled = enabled
        UserDefaults.standard.set(enabled, forKey: Self.key)
    }
}

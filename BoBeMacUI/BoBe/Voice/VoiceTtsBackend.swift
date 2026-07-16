import Foundation
import Observation

enum VoiceTtsBackend: String, CaseIterable, Sendable {
    case serverKokoro = "server_kokoro"
    case clientSupertonic = "client_supertonic"

    var label: String {
        L10n.tr("settings.voice.tts_backend.\(self.rawValue)")
    }
}

@MainActor
@Observable
final class VoiceTtsPreference {
    static let shared = VoiceTtsPreference()

    private static let key = "bobe.voice_tts_backend"

    private(set) var backend: VoiceTtsBackend

    private init() {
        self.backend = UserDefaults.standard.string(forKey: Self.key)
            .flatMap(VoiceTtsBackend.init(rawValue:)) ?? .serverKokoro
    }

    func setBackend(_ backend: VoiceTtsBackend) {
        self.backend = backend
        UserDefaults.standard.set(backend.rawValue, forKey: Self.key)
    }
}

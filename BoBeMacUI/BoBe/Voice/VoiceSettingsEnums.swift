import Foundation

/// Kokoro v1.0 multilingual voice slot table. Mirrored from the daemon's
/// `BoBeService/src/speech/providers/sherpa/kokoro_tts.rs::voice_id` enum.
///
/// **Split-brain warning**: there is no shared schema. Both lists must
/// update together when Kokoro publishes a new voice. The Rust side is
/// authoritative (the daemon rejects unknown ids); this Swift list
/// controls only the UI picker. The `voice_persona_round_trip` integration
/// test (when it lands) will assert every entry here is accepted by the
/// daemon.
enum KokoroVoices {
    static let allSlots: [String] = [
        "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica",
        "af_kore", "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky",
        "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam",
        "am_michael", "am_onyx", "am_puck", "am_santa",
        "bf_alice", "bf_emma", "bf_isabella", "bf_lily",
        "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
        "ef_dora", "em_alex",
        "ff_siwis",
        "hf_alpha", "hf_beta", "hm_omega", "hm_psi",
        "if_sara", "im_nicola",
        "jf_alpha", "jf_gongitsune", "jf_nezumi", "jf_tebukuro", "jm_kumo",
        "pf_dora", "pm_alex", "pm_santa",
        "zf_xiaobei", "zf_xiaoni", "zf_xiaoxiao", "zf_xiaoyi",
        "zm_yunjian", "zm_yunxi", "zm_yunxia", "zm_yunyang",
    ]

    /// Friendly display name for a slot (e.g. `af_bella` → "Bella (English ♀)").
    static func displayName(for slot: String) -> String {
        let parts = slot.split(separator: "_", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return slot }
        let prefix = parts[0]
        let name = parts[1].capitalized
        let lang = self.languageLabel(forPrefix: prefix)
        return "\(name) (\(lang))"
    }

    private static func languageLabel(forPrefix prefix: String) -> String {
        let lang = switch prefix.first {
        case "a": "English"
        case "b": "British English"
        case "e": "Spanish"
        case "f": "French"
        case "h": "Hindi"
        case "i": "Italian"
        case "j": "Japanese"
        case "p": "Portuguese"
        case "z": "Mandarin"
        default: "?"
        }
        let gender = switch prefix.last {
        case "f": "♀"
        case "m": "♂"
        default: ""
        }
        return gender.isEmpty ? lang : "\(lang) \(gender)"
    }
}

/// Languages exposed in the Settings → Voice picker. English uses
/// FluidAudio Parakeet EOU; everything else uses Nemotron multilingual
/// streaming ASR + Silero VAD.
enum VoiceLanguages {
    /// BCP-47 codes. Order surfaces shipping languages first.
    static let all: [String] = ["en", "zh", "es", "el", "ko", "ja"]

    static let shipping: Set = Set(VoiceLanguages.all)

    static func displayName(for code: String) -> String {
        let base: String = switch code {
        case "en": "English"
        case "zh": "Mandarin"
        case "es": "Spanish"
        case "el": "Greek"
        case "ko": "Korean"
        case "ja": "Japanese"
        default: code
        }
        return Self.shipping.contains(code) ? base : "\(base)  (coming soon)"
    }
}

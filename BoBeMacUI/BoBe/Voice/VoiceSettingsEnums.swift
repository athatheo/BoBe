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
        let lang: String
        switch prefix.first {
        case "a": lang = "English"
        case "b": lang = "British English"
        case "e": lang = "Spanish"
        case "f": lang = "French"
        case "h": lang = "Hindi"
        case "i": lang = "Italian"
        case "j": lang = "Japanese"
        case "p": lang = "Portuguese"
        case "z": lang = "Mandarin"
        default: lang = "?"
        }
        let gender: String
        switch prefix.last {
        case "f": gender = "♀"
        case "m": gender = "♂"
        default: gender = ""
        }
        return gender.isEmpty ? lang : "\(lang) \(gender)"
    }
}

/// Languages exposed in the Settings → Voice picker. The matrix in
/// `docs/voice-architecture.md` lists 6 — English + Mandarin ship today
/// via FluidAudio Parakeet EOU and Qwen3-ASR + VAD respectively; the rest
/// are queued behind Qwen3 wiring for those locales (Qwen3 itself supports
/// all six).
enum VoiceLanguages {
    /// BCP-47 codes. Order surfaces shipping languages first.
    static let all: [String] = ["en", "zh", "es", "el", "ko", "ja"]

    /// Languages that have a working STT engine wired up. Everything else
    /// falls through to "(coming soon)" in the picker.
    static let shipping: Set<String> = ["en", "zh"]

    static func displayName(for code: String) -> String {
        let base: String
        switch code {
        case "en": base = "English"
        case "zh": base = "Mandarin"
        case "es": base = "Spanish"
        case "el": base = "Greek"
        case "ko": base = "Korean"
        case "ja": base = "Japanese"
        default: base = code
        }
        return Self.shipping.contains(code) ? base : "\(base)  (coming soon)"
    }
}

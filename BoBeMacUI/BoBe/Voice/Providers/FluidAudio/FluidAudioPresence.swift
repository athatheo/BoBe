import Foundation

/// Static-only surface shared by every FluidAudio model presence type
/// (Parakeet today, Qwen3 today, future variants). Lets settings UI map
/// `VoicePipeline.SttStatus → VoiceModelCard.Status` once instead of per
/// engine. The 4 statics here are the entire contract a card needs.
protocol FluidAudioPresence {
    static func isInstalled() -> Bool
    static var approximateTotalBytes: UInt64 { get }
    static func observedBytes() -> UInt64
    static func observedPercent() -> Int
}

extension FluidAudioModelPresence: FluidAudioPresence {}
extension FluidAudioQwen3ModelPresence: FluidAudioPresence {}

import AVFoundation
import Foundation

/// A swappable speech-to-text engine on the Swift client side. `VoicePipeline`
/// owns one active engine per WS session, resolved from the user's chosen
/// language (`settings.voiceSttLanguage`).
///
/// Existing impls:
///   - `FluidAudioStt` — FluidAudio Parakeet EOU 120M (English; built-in EOU)
///   - `FluidAudioQwen3Stt` — FluidAudio Qwen3-ASR streaming + VadManager
///     (Mandarin; VAD-driven EOU)
///
/// Both impls are actors. The protocol's `async` requirements line up with
/// the actors' implicit `async` boundary from outside.
protocol VoiceSttEngine: Sendable {
    /// Load the model (downloads from HuggingFace on first run, then caches
    /// to `~/Library/Application Support/FluidAudio/Models/`). Idempotent
    /// and concurrent-safe — N callers cause one download.
    func loadModels(
        onPartial: @escaping @Sendable (String) -> Void,
        onEou: @escaping @Sendable (String) -> Void
    ) async throws

    /// Feed one PCM buffer. Engine handles its own format conversion
    /// internally — callers pass whatever the audio tap produces.
    /// `sending` lets a non-Sendable `AVAudioPCMBuffer` cross the actor
    /// boundary safely (caller transfers ownership).
    func acceptAudio(_ buffer: sending AVAudioPCMBuffer) async throws

    /// Flush buffered audio and return the final transcript. Used when the
    /// user explicitly stops the mic, not on natural EOU (which fires the
    /// `onEou` callback supplied at load time).
    func finish() async throws -> String

    /// Reset the decoder/buffer between turns. Keeps models loaded.
    func reset() async throws

    /// Tear down model memory. Requires fresh `loadModels()` to reuse.
    func cleanup() async
}

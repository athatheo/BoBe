import AVFoundation
import Foundation

/// Swappable client-side STT engine. VoicePipeline picks one per WS based
/// on `settings.voiceSttLanguage`. Impls: `FluidAudioStt` (Parakeet,
/// English) and `FluidAudioQwen3Stt` (Qwen3 + VAD, Mandarin).
protocol VoiceSttEngine: Sendable {
    /// Idempotent + concurrent-safe; N callers → one HuggingFace download
    /// into `~/Library/Application Support/FluidAudio/Models/`.
    func loadModels(
        onPartial: @escaping @Sendable (String) -> Void,
        onEou: @escaping @Sendable (String) -> Void
    ) async throws

    /// Feed one PCM buffer. `sending` transfers ownership across the actor
    /// boundary; engine does its own format conversion.
    func acceptAudio(_ buffer: sending AVAudioPCMBuffer) async throws

    /// Flush + return transcript. For explicit mic-stop; natural EOU goes
    /// through the `onEou` callback supplied at load time.
    func finish() async throws -> String

    /// Reset decoder/buffer between turns; keeps models loaded.
    func reset() async throws

    /// Tear down model memory. Requires fresh `loadModels()` to reuse.
    func cleanup() async
}

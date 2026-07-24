import Foundation

/// Swappable client-side STT engine. VoicePipeline picks one per WS based
/// on `settings.voiceSttLanguage`. Impls: `FluidAudioStt` (Parakeet,
/// English) and `FluidAudioNemotronStt` (Nemotron + VAD, multilingual).
protocol VoiceSttEngine: Sendable {
    /// Idempotent + concurrent-safe; N callers → one HuggingFace download
    /// into `~/Library/Application Support/FluidAudio/Models/`.
    func loadModels(
        onPartial: @escaping @Sendable (String) async -> Void,
        onEou: @escaping @Sendable (String) async -> Void,
        onProgress: @escaping @Sendable (VoiceSttLoadProgress) -> Void
    ) async throws

    /// Cancel an in-flight model download/compile while preserving valid cache
    /// entries already written by FluidAudio's ModelHub.
    func cancelLoading() async

    /// Feed normalized 16 kHz mono Float32 samples from the shared input
    /// processor. Implementations never resample independently.
    func acceptSamples(_ samples: [Float]) async throws

    /// Flush + return transcript. For explicit mic-stop; natural EOU goes
    /// through the `onEou` callback supplied at load time.
    func finish() async throws -> String

    /// Reset decoder/buffer between turns; keeps models loaded.
    func reset() async throws

    /// Tear down model memory. Requires fresh `loadModels()` to reuse.
    func cleanup() async
}

enum VoiceSttCallbackEvent: Sendable {
    case partial(String)
    case endOfUtterance(String)
    case barrier(CheckedContinuation<Void, Never>)
}

struct VoiceSttLoadProgress: Sendable, Equatable {
    enum Phase: Sendable, Equatable {
        case listing
        case downloading(completedFiles: Int, totalFiles: Int)
        case compiling(modelName: String)
    }

    let fractionCompleted: Double
    let phase: Phase

    var percent: Int {
        max(0, min(100, Int((self.fractionCompleted * 100).rounded())))
    }
}

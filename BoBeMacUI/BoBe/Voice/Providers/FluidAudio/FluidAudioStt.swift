import AVFoundation
import Foundation
import FluidAudio
import OSLog

/// Thin wrapper around FluidAudio's `StreamingEouAsrManager` (Parakeet EOU
/// 120M, English). Drives streaming partials + an end-of-utterance callback
/// that fires when the user stops speaking. VoicePipeline pushes 16kHz mono
/// Float32 audio in via `appendAudio` and listens on the callbacks.
///
/// Concurrency: this is an actor. Callbacks invoke into the supplied
/// `@Sendable` closures from FluidAudio's internal queue; consumers should
/// hop to `@MainActor` at the UI boundary (VoicePipeline already does).
actor FluidAudioStt: VoiceSttEngine {
    private let logger = Logger(subsystem: "com.bobe.app", category: "FluidAudioStt")
    private let variant: StreamingModelVariant
    private let eouDebounceMs: Int
    private var manager: StreamingEouAsrManager?
    private var loaded = false
    /// In-flight load task. Concurrent `loadModels` callers (e.g. MicButton
    /// view-appear + user-tap firing prewarm twice) await the same task
    /// instead of triggering N parallel downloads.
    private var loadTask: Task<Void, Error>?

    /// Variant tuning — `.parakeetEou320ms` is the balanced default. The
    /// 160ms variant is lowest-latency; 1280ms is highest-throughput.
    init(
        variant: StreamingModelVariant = .parakeetEou320ms,
        eouDebounceMs: Int = 1280
    ) {
        self.variant = variant
        self.eouDebounceMs = eouDebounceMs
    }

    /// Load the model (downloads from HuggingFace on first run, then caches
    /// to `~/Library/Application Support/FluidAudio/Models/`). Idempotent
    /// and concurrent-safe — N callers cause one download.
    func loadModels(
        onPartial: @escaping @Sendable (String) -> Void,
        onEou: @escaping @Sendable (String) -> Void
    ) async throws {
        if self.loaded { return }
        if let existing = self.loadTask {
            try await existing.value
            return
        }
        let variant = self.variant
        let task = Task<Void, Error> { [weak self] in
            guard let self else { return }
            try await self.performLoad(variant: variant, onPartial: onPartial, onEou: onEou)
        }
        self.loadTask = task
        do {
            try await task.value
        } catch {
            self.loadTask = nil
            throw error
        }
    }

    /// Actual load work, run inside the deduped Task.
    private func performLoad(
        variant: StreamingModelVariant,
        onPartial: @escaping @Sendable (String) -> Void,
        onEou: @escaping @Sendable (String) -> Void
    ) async throws {
        let mgr = variant.createManager() as? StreamingEouAsrManager
        guard let mgr else {
            throw FluidAudioSttError.unexpectedManagerType
        }
        await mgr.setPartialCallback(onPartial)
        await mgr.setEouCallback(onEou)
        try await mgr.loadModels()
        self.manager = mgr
        self.loaded = true
        self.loadTask = nil
        self.logger.info("FluidAudioStt loaded \(variant.rawValue)")
    }

    /// Append one PCM buffer (any format — FluidAudio resamples internally
    /// to 16kHz mono Float32) and drive a processing pass.
    /// `sending` parameter so AVAudioPCMBuffer (non-Sendable) can cross the
    /// actor boundary safely — the caller transfers ownership.
    func acceptAudio(_ buffer: sending AVAudioPCMBuffer) async throws {
        guard let mgr = self.manager else {
            throw FluidAudioSttError.notLoaded
        }
        try await mgr.appendAudio(buffer)
        try await mgr.processBufferedAudio()
    }

    /// Flush any buffered audio and return the final transcript.
    /// Used when the user explicitly stops the mic (not on natural EOU,
    /// which fires the `onEou` callback supplied at load time).
    func finish() async throws -> String {
        guard let mgr = self.manager else { return "" }
        return try await mgr.finish()
    }

    /// Reset the decoder + buffer between turns. Keeps models loaded.
    func reset() async throws {
        guard let mgr = self.manager else { return }
        await mgr.reset()
    }

    /// Tear down — release model memory. Cannot be reused without a fresh
    /// `loadModels` call.
    func cleanup() async {
        guard let mgr = self.manager else { return }
        await mgr.cleanup()
        self.manager = nil
        self.loaded = false
    }
}

enum FluidAudioSttError: Error {
    case notLoaded
    case unexpectedManagerType
}

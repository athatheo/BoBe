import AVFoundation
import FluidAudio
import Foundation
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
    private let chunkSize: StreamingChunkSize
    private var eouDebounceMs: Int
    private var manager: StreamingEouAsrManager?
    private var loaded = false
    /// In-flight load task. Concurrent `loadModels` callers (e.g. MicButton
    /// view-appear + user-tap firing prewarm twice) await the same task
    /// instead of triggering N parallel downloads.
    private var loadTask: Task<Void, Error>?
    /// Wraps the raw partial stream with punctuation-aware commit logic
    /// (FluidAudio 0.13.5+). Caption surfaces punctuated text rather
    /// than a punctuation-free run; EOU's final transcript benefits too.
    private let commitLayer = PunctuationCommitLayer()
    /// Hoisted so the FluidAudio callbacks (which fire on FluidAudio's
    /// executor) can bounce into our actor and forward in order.
    private var savedOnPartial: (@Sendable (String) -> Void)?
    private var savedOnEou: (@Sendable (String) -> Void)?

    /// Variant tuning — `.ms320` is the balanced default. The 160ms variant
    /// is lowest-latency; 1280ms is highest-throughput.
    /// `eouDebounceMs` defaults to Parakeet's documented "balanced" value;
    /// callers update it via `setEouDebounceMs(_:)` to honor the user's
    /// pause sensitivity preference.
    init(
        chunkSize: StreamingChunkSize = .ms320,
        eouDebounceMs: Int = 1280
    ) {
        self.chunkSize = chunkSize
        self.eouDebounceMs = eouDebounceMs
    }

    /// Update the EOU debounce. Picked up on the next loaded manager
    /// (next reload) — the live manager's value also gets set if loaded.
    func setEouDebounceMs(_ value: Int) async {
        self.eouDebounceMs = max(100, min(5_000, value))
        if let mgr = self.manager {
            await mgr.updateEouDebounceMs(self.eouDebounceMs)
        }
    }

    /// Routes one FluidAudio partial through PunctuationCommitLayer and
    /// forwards the punctuated text. Actor-isolated, so concurrent
    /// callback fires from FluidAudio's executor land in arrival order.
    private func handleRawPartial(_ text: String) async {
        let update = await self.commitLayer.processPartialText(text)
        self.savedOnPartial?(update.totalText)
    }

    /// EOU equivalent — flushes the commit layer; prefers its committed
    /// text when non-empty (more reliable punctuation than the raw end).
    private func handleRawEou(_ text: String) async {
        let update = await self.commitLayer.processEOU()
        let final = update.committedText.isEmpty ? text : update.committedText
        self.savedOnEou?(final)
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
        let chunkSize = self.chunkSize
        let debounceMs = self.eouDebounceMs
        let task = Task<Void, Error> { [weak self] in
            guard let self else { return }
            try await self.performLoad(
                chunkSize: chunkSize,
                debounceMs: debounceMs,
                onPartial: onPartial,
                onEou: onEou
            )
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
        chunkSize: StreamingChunkSize,
        debounceMs: Int,
        onPartial: @escaping @Sendable (String) -> Void,
        onEou: @escaping @Sendable (String) -> Void
    ) async throws {
        // Build StreamingEouAsrManager directly so we can pass the user's
        // pause-sensitivity-derived `eouDebounceMs` — the variant factory
        // (`StreamingModelVariant.createManager()`) hard-codes 1280ms.
        let mgr = StreamingEouAsrManager(
            chunkSize: chunkSize,
            eouDebounceMs: debounceMs
        )
        // Route FluidAudio's raw callbacks through OUR actor first so the
        // commit-layer hops stay serialized. Spawning detached Tasks
        // directly in the FluidAudio callback risks Task-scheduling
        // reordering partials → out-of-order commit-layer state.
        self.savedOnPartial = onPartial
        self.savedOnEou = onEou
        await mgr.setPartialCallback { [weak self] text in
            Task { await self?.handleRawPartial(text) }
        }
        await mgr.setEouCallback { [weak self] text in
            Task { await self?.handleRawEou(text) }
        }
        try await mgr.loadModels()
        self.manager = mgr
        self.loaded = true
        self.loadTask = nil
        self.logger.info("FluidAudioStt loaded chunk=\(chunkSize.durationMs)ms eou=\(debounceMs)ms")
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
        await self.commitLayer.reset()
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
}

/// Cross-actor write helper for the EOU debounce. FluidAudio exposes
/// `eouDebounceMs` as a `public var` but no setter method, so writing it
/// from outside the actor requires going through an isolated method on
/// the actor — which extensions of actors get for free.
extension StreamingEouAsrManager {
    public func updateEouDebounceMs(_ value: Int) {
        self.eouDebounceMs = value
    }
}

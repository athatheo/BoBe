@preconcurrency import AVFoundation
import FluidAudio
import Foundation
import OSLog

/// Qwen3-ASR streaming wrapper for non-English languages. Pairs
/// `Qwen3StreamingManager` with FluidAudio's Silero VadManager; speechEnd
/// schedules an `eouDelayMs` timer that fires EOU unless speechStart
/// arrives first. Callbacks hop to @MainActor at the UI boundary.
@available(macOS 15, iOS 18, *)
actor FluidAudioQwen3Stt: VoiceSttEngine {
    private let logger = Logger(subsystem: "com.bobe.app", category: "FluidAudioQwen3Stt")
    private var language: Qwen3AsrConfig.Language
    private var streamingConfig: Qwen3StreamingConfig
    private var eouDelayMs: Int

    private var asrManager: Qwen3AsrManager?
    private var streaming: Qwen3StreamingManager?
    private var vadManager: VadManager?
    private var vadState: VadStreamState = .initial()
    /// Wraps the raw partial stream with punctuation-aware commit logic
    /// (FluidAudio 0.13.5+). Produces a `totalText` we surface as the
    /// live caption — sentences are punctuated as the user speaks
    /// rather than waiting for EOU to add a period.
    private let commitLayer = PunctuationCommitLayer()

    private var onPartial: (@Sendable (String) -> Void)?
    private var onEou: (@Sendable (String) -> Void)?

    private var loaded = false
    /// In-flight load task. Concurrent `loadModels` callers (e.g. wizard
    /// + mic-button prewarm racing at startup) await the same task instead
    /// of triggering parallel downloads of the ~1.75GB Qwen3 bundle.
    private var loadTask: Task<Void, Error>?

    /// VAD-driven EOU timer. Cancelled on every speechStart; fires after
    /// `eouDelayMs` of trailing silence following the last speechEnd.
    private var eouTimer: Task<Void, Never>?

    /// Lazy converter from the input format (whatever the audio tap
    /// produces) to 16kHz mono Float32. Both Qwen3 + Silero VAD want raw
    /// `[Float]` so we extract once per `acceptAudio` call.
    private var converter: AVAudioConverter?
    private var targetFormat: AVAudioFormat?

    init(
        language: Qwen3AsrConfig.Language = .chinese,
        eouDelayMs: Int = 800
    ) {
        self.language = language
        self.eouDelayMs = eouDelayMs
        self.streamingConfig = Qwen3StreamingConfig(
            minAudioSeconds: 1.0,
            chunkSeconds: 1.0,
            maxAudioSeconds: 30.0,
            language: language
        )
    }

    /// Update the silence-after-speech delay before EOU fires. Picked up
    /// on the NEXT speechEnd event — already-scheduled timers keep their
    /// original delay so a mid-utterance settings change doesn't cut off
    /// the user.
    func setEouDelayMs(_ value: Int) {
        self.eouDelayMs = max(100, min(5_000, value))
    }

    /// Switch Qwen3's streaming language for the next session. Qwen3 is a
    /// multilingual checkpoint, so the model itself doesn't reload — only
    /// the streaming config swaps. If already loaded, the active streaming
    /// manager is reconfigured in place via `configure(_:)`.
    func setLanguage(_ language: Qwen3AsrConfig.Language) {
        if self.language == language { return }
        self.language = language
        self.streamingConfig = Qwen3StreamingConfig(
            minAudioSeconds: 1.0,
            chunkSeconds: 1.0,
            maxAudioSeconds: 30.0,
            language: language
        )
        Task { [weak self] in
            await self?.streaming?.configure(self?.streamingConfig ?? .default)
        }
    }

    func loadModels(
        onPartial: @escaping @Sendable (String) -> Void,
        onEou: @escaping @Sendable (String) -> Void
    ) async throws {
        if self.loaded { return }
        if let existing = self.loadTask {
            try await existing.value
            return
        }
        self.onPartial = onPartial
        self.onEou = onEou
        let task = Task<Void, Error> { [weak self] in
            try await self?.performLoad()
        }
        self.loadTask = task
        do {
            try await task.value
        } catch {
            self.loadTask = nil
            throw error
        }
    }

    private func performLoad() async throws {
        // Qwen3 ASR — download (~1.75 GB f32) then load into a 2-model
        // CoreML pipeline. Uses default cache root so the location matches
        // `FluidAudioQwen3ModelPresence` and other FluidAudio consumers.
        let qwen3Dir = try await Qwen3AsrModels.download(variant: .f32)
        let asr = Qwen3AsrManager()
        try await asr.loadModels(from: qwen3Dir)
        let stream = Qwen3StreamingManager(asrManager: asr, config: self.streamingConfig)

        // Silero VAD — small (~3 MB) cohort model that fires speechStart /
        // speechEnd events; drives our EOU timer.
        let vad = try await VadManager()
        let initialVadState = await vad.makeStreamState()

        self.asrManager = asr
        self.streaming = stream
        self.vadManager = vad
        self.vadState = initialVadState
        self.loaded = true
        self.loadTask = nil
        self.logger.info("FluidAudioQwen3Stt loaded for language \(self.language.rawValue, privacy: .public)")
    }

    func acceptAudio(_ buffer: sending AVAudioPCMBuffer) async throws {
        guard let stream = self.streaming, let vad = self.vadManager else {
            throw FluidAudioQwen3SttError.notLoaded
        }
        let samples = try self.extractFloat16kSamples(from: buffer)
        guard !samples.isEmpty else { return }

        // VAD first — drives the EOU timer.
        let vadResult = try await vad.processStreamingChunk(samples, state: self.vadState)
        self.vadState = vadResult.state
        if let event = vadResult.event {
            self.handleVadEvent(event)
        }

        // Qwen3 — emits a partial when its internal accumulator decides
        // enough audio has come in for a re-transcribe pass. Run the raw
        // partial through PunctuationCommitLayer so the caption surfaces
        // sentence-punctuated text rather than a punctuation-free run.
        if let result = try await stream.addAudio(samples), !result.transcript.isEmpty {
            let update = await self.commitLayer.processPartialText(result.transcript)
            self.onPartial?(update.totalText)
        }
    }

    private func handleVadEvent(_ event: VadStreamEvent) {
        switch event.kind {
        case .speechStart:
            self.eouTimer?.cancel()
            self.eouTimer = nil
        case .speechEnd:
            self.scheduleEou()
        }
    }

    private func scheduleEou() {
        self.eouTimer?.cancel()
        let delayMs = self.eouDelayMs
        self.eouTimer = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(delayMs))
            guard !Task.isCancelled else { return }
            await self?.fireEou()
        }
    }

    private func fireEou() async {
        self.eouTimer = nil
        guard let stream = self.streaming else { return }
        do {
            let result = try await stream.finish()
            let update = await self.commitLayer.processEOU()
            // Prefer the commit-layer's punctuated text when it produced
            // anything; fall back to the raw Qwen3 transcript otherwise.
            let final = update.committedText.isEmpty ? result.transcript : update.committedText
            self.onEou?(final)
        } catch {
            self.logger.error("Qwen3 finish failed: \(error.localizedDescription, privacy: .public)")
            self.onEou?("")
        }
    }

    func finish() async throws -> String {
        self.eouTimer?.cancel()
        self.eouTimer = nil
        guard let stream = self.streaming else { return "" }
        let result = try await stream.finish()
        return result.transcript
    }

    func reset() async throws {
        self.eouTimer?.cancel()
        self.eouTimer = nil
        await self.streaming?.reset()
        await self.commitLayer.reset()
        if let vad = self.vadManager {
            self.vadState = await vad.makeStreamState()
        }
    }

    func cleanup() async {
        self.eouTimer?.cancel()
        self.eouTimer = nil
        self.streaming = nil
        self.asrManager = nil
        self.vadManager = nil
        self.loaded = false
    }

    // MARK: - Audio conversion

    /// Extract 16kHz mono Float32 samples from whatever the audio tap
    /// delivered. Caches the converter on first call so subsequent buffers
    /// don't pay the setup cost; channel-maps to the AEC'd channel 0 only
    /// when VPIO delivers multi-channel input.
    private func extractFloat16kSamples(from buffer: AVAudioPCMBuffer) throws -> [Float] {
        let srcFmt = buffer.format
        let isNativeFormat = srcFmt.sampleRate == 16_000
            && srcFmt.channelCount == 1
            && srcFmt.commonFormat == .pcmFormatFloat32
        if isNativeFormat {
            guard let data = buffer.floatChannelData?[0] else { return [] }
            return Array(UnsafeBufferPointer(start: data, count: Int(buffer.frameLength)))
        }

        let targetFmt: AVAudioFormat
        if let cached = self.targetFormat {
            targetFmt = cached
        } else {
            guard let fmt = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: 16_000,
                channels: 1,
                interleaved: false
            )
            else {
                throw FluidAudioQwen3SttError.audioConversionFailed
            }
            self.targetFormat = fmt
            targetFmt = fmt
        }

        let conv: AVAudioConverter
        if let cached = self.converter {
            conv = cached
        } else {
            guard let fresh = AVAudioConverter(from: srcFmt, to: targetFmt) else {
                throw FluidAudioQwen3SttError.audioConversionFailed
            }
            if srcFmt.channelCount > 1 {
                fresh.channelMap = [NSNumber(value: 0)]
            }
            self.converter = fresh
            conv = fresh
        }

        let outCap = AVAudioFrameCount(
            Double(buffer.frameLength) * targetFmt.sampleRate / srcFmt.sampleRate
        ) + 16
        guard let outBuf = AVAudioPCMBuffer(pcmFormat: targetFmt, frameCapacity: outCap) else {
            throw FluidAudioQwen3SttError.audioConversionFailed
        }

        let (status, err) = convertSingleBuffer(conv, source: buffer, into: outBuf)
        if status == .error || err != nil {
            throw FluidAudioQwen3SttError.audioConversionFailed
        }
        guard let data = outBuf.floatChannelData?[0] else { return [] }
        return Array(UnsafeBufferPointer(start: data, count: Int(outBuf.frameLength)))
    }
}

@available(macOS 15, iOS 18, *)
enum FluidAudioQwen3SttError: Error {
    case notLoaded
    case audioConversionFailed
}

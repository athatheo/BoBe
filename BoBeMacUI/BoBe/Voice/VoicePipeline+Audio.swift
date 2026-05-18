@preconcurrency import AVFoundation
import FluidAudio
import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "VoicePipeline.Audio")

/// AVAudioEngine setup + per-frame input handling.
///
/// **Why this lives in an extension.** `VoicePipeline.swift` would
/// otherwise blow past SwiftLint's 800-line file cap (and reading it
/// front-to-back would mean swimming through audio engine internals before
/// reaching the WS lifecycle). The audio chain is self-contained — engine
/// configure / teardown, the input tap, RMS publishing, barge-in
/// detection. All called from the same realtime audio dispatch queue and
/// hopped to MainActor at the tap boundary.
///
/// Mirror of the `+WebSocket` and `+AudioDevice` extensions: same
/// `@MainActor` isolation, same conventions, same access surface.
@MainActor
extension VoicePipeline {
    // MARK: - Engine setup

    /// Reverses `configureEngine`. Idempotent — safe to call when not
    /// warm. `internal` so the audio-device hot-swap path in
    /// `+AudioDevice` can drive the rebuild cycle.
    func tearDownAudio() {
        guard self.isWarm else { return }
        self.audioEngine.inputNode.removeTap(onBus: 0)
        self.playerNode.removeTap(onBus: 0)
        self.playerNode.stop()
        self.audioEngine.stop()
        // Explicitly disable VPIO on both nodes AFTER stopping the engine
        // (Apple requires a stopped engine to toggle this). Without these
        // calls macOS keeps the VPIO AudioUnit's "duck other audio"
        // behaviour active across the engine's stopped state, so
        // Spotify/etc. stay quiet even though the mic is off. Toggling
        // it off is what actually releases the system-wide duck.
        // Wrapped in try? — these can't fail in practice (the engine is
        // stopped, we never touched audio session state on macOS), but
        // throwing here would leak isWarm=true and break re-connect.
        try? self.audioEngine.inputNode.setVoiceProcessingEnabled(false)
        try? self.audioEngine.outputNode.setVoiceProcessingEnabled(false)
        self.converter = nil
        self.pcmAccumulator.removeAll(keepingCapacity: false)
        self.pendingTurnId = nil
        // STT model itself stays loaded across reconnects; just clear state.
        let engine = self.activeStt
        Task { try? await engine.reset() }
        self.bargeInCount = 0
        self.bargeInSent = false
        self.isWarm = false
    }

    /// `internal` so the audio-device hot-swap extension can rebuild the
    /// engine after a default-input change.
    func configureEngine() throws {
        let inputNode = self.audioEngine.inputNode
        try inputNode.setVoiceProcessingEnabled(true)
        // VPIO on the output node too — gives Apple's AEC a reference
        // signal for AEC during full-duplex (barge-in path). Required by
        // the architecture doc; on macOS it's idempotent if already
        // enabled.
        try self.audioEngine.outputNode.setVoiceProcessingEnabled(true)

        let inputFormat = inputNode.inputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0 else {
            throw VoiceError.runtime("input sample rate is 0 — no mic device?")
        }
        guard let int16Format = AVAudioFormat(
            commonFormat: .pcmFormatInt16,
            sampleRate: self.captureSampleRate,
            channels: 1,
            interleaved: false
        )
        else {
            throw VoiceError.runtime("could not create Int16 target format")
        }
        guard let converter = AVAudioConverter(from: inputFormat, to: int16Format) else {
            throw VoiceError.runtime("could not create AVAudioConverter")
        }
        // VPIO on macOS delivers multichannel deinterleaved Float32.
        // Channel 0 is the AEC'd mic; channels 1+ are reference signals.
        // Default downmix would sum them all and mask AEC. Force ch 0 only.
        if inputFormat.channelCount > 1 {
            converter.channelMap = [NSNumber(value: 0)]
        }
        self.converter = converter

        // installTap's block runs on the realtime audio dispatch queue.
        // Marked @Sendable so it doesn't inherit @MainActor isolation
        // from the enclosing class — without this, Swift 6's runtime
        // isolation check crashes the realtime queue the moment audio
        // flows.
        let tapBlock: @Sendable (AVAudioPCMBuffer, AVAudioTime) -> Void = { [weak self] buffer, _ in
            DispatchQueue.main.async {
                self?.handleInputBuffer(buffer)
            }
        }
        inputNode.installTap(onBus: 0, bufferSize: 2048, format: inputFormat, block: tapBlock)

        // TTS playback envelope tap. Decoding all incoming Opus frames
        // takes ~200ms while the actual playback through the speakers
        // takes seconds. Without a real-time tap, ttsOutputLevel would
        // peak briefly at decode time and immediately decay to 0 while
        // the avatar's mouth still needs to be moving. Tap the player
        // node directly so we sample what is actually leaving the
        // speakers — full duration, in sync with what the user hears.
        let playerFormat = self.playerNode.outputFormat(forBus: 0)
        if playerFormat.sampleRate > 0 {
            let playerTapBlock: @Sendable (AVAudioPCMBuffer, AVAudioTime) -> Void = { [weak self] buffer, _ in
                let rms = computePlaybackRms(buffer)
                guard rms.isFinite else { return }
                DispatchQueue.main.async {
                    self?.handlePlaybackRms(rms)
                }
            }
            self.playerNode.installTap(
                onBus: 0,
                bufferSize: 1_024,
                format: playerFormat,
                block: playerTapBlock
            )
        }

        self.audioEngine.prepare()
        try self.audioEngine.start()
        self.playerNode.play()
        // Now that the engine is alive, ensure the CoreAudio default-
        // input/output listeners are wired so we react to mid-session
        // device changes (AirPods plug-in, USB mic swap, System Settings
        // input/output flip). Idempotent — registers once per process.
        self.registerInputDeviceObserverIfNeeded()
        logger.info("voice engine configured; input \(describe(inputFormat), privacy: .public)")
    }

    // MARK: - Tap handling

    /// Realtime mic-tap handler, hopped to MainActor by the tap block.
    /// Runs at ~50 Hz during active capture — every microsecond of work
    /// here is paid back many times over.
    func handleInputBuffer(_ buffer: AVAudioPCMBuffer) {
        guard self.task != nil,
              let converter = self.converter
        else { return }
        switch self.state {
        case .idle, .connecting, .failed:
            return
        case .listening, .capturing, .thinking, .speaking, .cancelling:
            break
        }

        let outFormat = converter.outputFormat
        let cap = AVAudioFrameCount(
            Double(buffer.frameLength) * outFormat.sampleRate / buffer.format.sampleRate
        ) + 16
        guard let outBuf = AVAudioPCMBuffer(pcmFormat: outFormat, frameCapacity: cap) else { return }

        let (status, err) = convertSingleBuffer(converter, source: buffer, into: outBuf)
        if status == .error || err != nil {
            self.lastError = "convert: \(err?.localizedDescription ?? "?")"
            logger.error("convert: \(err?.localizedDescription ?? "?", privacy: .public)")
            return
        }
        guard let int16Ptr = outBuf.int16ChannelData?[0] else { return }
        let count = Int(outBuf.frameLength)
        let samples = UnsafeBufferPointer(start: int16Ptr, count: count)
        self.pcmAccumulator.append(contentsOf: samples)

        // Walk the accumulator in 320-sample windows via index offset
        // rather than `prefix(...) + removeFirst(...)`. The old pattern
        // allocated a fresh [Int16] each iteration and forced an O(n)
        // memmove on the remaining samples — multiply by ~50 Hz tap
        // callbacks and the wasted work was visible in instruments.
        var readIndex = 0
        let frameSize = self.rmsFrameSamples
        while readIndex + frameSize <= self.pcmAccumulator.count {
            let slice = self.pcmAccumulator[readIndex ..< readIndex + frameSize]
            let frameRms = self.rmsDbfs(of: slice)
            self.publishInputLevel(frameRms: frameRms)
            self.checkBargeIn(frameRms: frameRms)
            readIndex += frameSize
        }
        if readIndex > 0 {
            self.pcmAccumulator.removeFirst(readIndex)
        }

        // Feed the ORIGINAL Float32 buffer to FluidAudio — each engine
        // resamples internally (Parakeet) or via its own AVAudioConverter
        // (Qwen3) so we don't need to pre-convert. Mute gates the feed
        // too so we don't transcribe during user-requested silence.
        guard !self.muted else { return }
        // Skip the feed entirely while the STT engine is still loading —
        // it can't accept buffers yet and would throw .notLoaded for
        // every 21ms chunk, spamming the main actor at ~50 logs/s. That
        // backlog is what stutters the avatar's voice presence ring at
        // first mic open. The engine drains buffered audio once .ready.
        guard self.sttStatus == .ready, self.sttLoaded else { return }
        // Deep-copy the buffer before the async hop. Apple's docs say
        // tap buffer storage may be reused after the installTap block
        // returns; capturing the buffer across `Task { ... await }`
        // would let the realtime queue clobber the bytes under heavy
        // CPU load before FluidAudio reads them. Helper lives in
        // VoicePipelineSupport.
        guard let copy = copyPcmFloatBuffer(buffer) else {
            logger.warning("stt.buffer_copy_failed")
            return
        }
        let engine = self.activeStt
        Task { [copy] in
            do {
                try await engine.acceptAudio(copy)
            } catch {
                // FluidAudio errors during a streaming pass are non-
                // fatal — most likely a transient model state issue.
                // Log and keep capturing; the next chunk's
                // processBufferedAudio will recover. .notLoaded
                // specifically can race with the readiness gate above
                // when the actor toggles loaded mid-chunk; demote to
                // debug so it doesn't drown the console.
                if case FluidAudioSttError.notLoaded = error {
                    logger.debug("stt.acceptAudio_notLoaded_race")
                } else {
                    logger.warning(
                        "stt.acceptAudio failed: \(error.localizedDescription, privacy: .public)"
                    )
                }
            }
        }
    }

    // MARK: - RMS, level publishing, barge-in

    /// Map dBFS (-60..0) → 0...1 with a soft floor so quiet background
    /// hum reads as ~0 and only deliberate speech moves the ring.
    private func publishInputLevel(frameRms: Float) {
        let floor: Float = -50
        let normalized = max(0, min(1, (frameRms - floor) / -floor))
        // Light EMA smoothing — 60Hz updates of raw RMS look jittery.
        let smoothed = (self.inputLevel * 0.6) + (normalized * 0.4)
        // Suppress sub-perceptual writes so @Observable doesn't
        // invalidate the avatar ring on every 20 ms frame; the ring
        // stroke costs 96 sin/cos per render so spurious writes are not
        // free.
        if abs(smoothed - self.inputLevel) > 0.005 {
            self.inputLevel = smoothed
        }
    }

    /// Generic over the slice type so callers can pass `ArraySlice<Int16>`
    /// without copying into a fresh `[Int16]`.
    private func rmsDbfs(of samples: some Collection<Int16>) -> Float {
        guard !samples.isEmpty else { return -100 }
        var sumSquares: Double = 0
        for s in samples {
            let f = Double(s) / 32_768.0
            sumSquares += f * f
        }
        let rms = sqrt(sumSquares / Double(samples.count))
        return rms > 1e-9 ? Float(20 * log10(rms)) : -100
    }

    /// Per-frame barge-in detector. Only fires during `.speaking`;
    /// resets in every other state so we don't carry stale counts
    /// between turns. Sends `barge_in` with the current playback
    /// position so the daemon can truncate the persisted assistant turn
    /// to what the user actually heard.
    private func checkBargeIn(frameRms: Float) {
        guard self.state == .speaking else {
            self.bargeInCount = 0
            self.bargeInSent = false
            return
        }
        if frameRms > self.bargeInRmsDbfs {
            self.bargeInCount += 1
            if self.bargeInCount >= self.bargeInFramesNeeded, !self.bargeInSent {
                self.bargeInSent = true
                let playedMs = self.currentPlaybackMs()
                let tsMs = UInt64(Date.now.timeIntervalSince1970 * 1_000)
                logger.info("voice.barge_in_sending playedMs=\(playedMs)")
                Task { [weak self] in
                    await self?.sendClient(.bargeIn(tsMs: tsMs, playbackMsPlayed: playedMs))
                }
            }
        } else {
            self.bargeInCount = 0
        }
    }

    /// Best-effort current playback position in ms, derived from the
    /// player node's render clock. ~100ms staleness is fine for the
    /// truncation math the daemon uses to persist the partial turn.
    private func currentPlaybackMs() -> UInt64 {
        let frames = self.playedFramesThisTurn()
        let ms = Double(frames) * 1_000.0 / self.playbackSampleRate
        return UInt64(max(0, ms))
    }
}

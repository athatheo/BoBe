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
        if self.inputTapInstalled {
            self.audioEngine.inputNode.removeTap(onBus: 0)
            self.inputTapInstalled = false
        }
        if self.playerTapInstalled {
            self.playerNode.removeTap(onBus: 0)
            self.playerTapInstalled = false
        }
        self.playerNode.stop()
        self.cancelClientTtsTurn()
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
        if self.inputVoiceProcessingEnabled {
            try? self.audioEngine.inputNode.setVoiceProcessingEnabled(false)
            self.inputVoiceProcessingEnabled = false
        }
        if self.outputVoiceProcessingEnabled {
            try? self.audioEngine.outputNode.setVoiceProcessingEnabled(false)
            self.outputVoiceProcessingEnabled = false
        }
        self.audioInputContinuation?.finish()
        self.audioInputContinuation = nil
        self.audioInputTask?.cancel()
        self.audioInputTask = nil
        Task { await self.inputProcessor.reset() }
        self.pendingTurnId = nil
        self.partialSendTask?.cancel()
        self.partialSendTask = nil
        self.pendingPartialForServer = nil
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
        do {
            try self.configureEngineTransaction()
        } catch {
            self.tearDownAudio()
            throw error
        }
    }

    private func configureEngineTransaction() throws {
        let inputNode = self.audioEngine.inputNode
        try inputNode.setVoiceProcessingEnabled(true)
        self.inputVoiceProcessingEnabled = true
        // VPIO on the output node too — gives Apple's AEC a reference
        // signal for AEC during full-duplex (barge-in path). Required by
        // the architecture doc; on macOS it's idempotent if already
        // enabled.
        try self.audioEngine.outputNode.setVoiceProcessingEnabled(true)
        self.outputVoiceProcessingEnabled = true

        let inputFormat = inputNode.inputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0 else {
            throw VoiceError.runtime("input sample rate is 0 — no mic device?")
        }

        let (audioStream, continuation) = AsyncStream.makeStream(
            of: CapturedVoiceAudio.self,
            bufferingPolicy: .bufferingNewest(16)
        )
        self.audioInputContinuation = continuation
        let engine = self.activeStt
        let processor = self.inputProcessor
        self.audioInputTask = Task { @MainActor [weak self] in
            for await captured in audioStream {
                guard !Task.isCancelled else { return }
                do {
                    let processed = try await processor.process(captured)
                    guard let self else { return }
                    await self.handleProcessedInput(processed, engine: engine)
                } catch {
                    guard let self else { return }
                    self.lastError = "input processing: \(error.localizedDescription)"
                    logger.warning(
                        "input processing failed: \(error.localizedDescription, privacy: .public)"
                    )
                }
            }
        }

        // installTap's block runs on the realtime audio dispatch queue.
        // Marked @Sendable so it doesn't inherit @MainActor isolation
        // from the enclosing class — without this, Swift 6's runtime
        // isolation check crashes the realtime queue the moment audio
        // flows.
        let tapBlock: @Sendable (AVAudioPCMBuffer, AVAudioTime) -> Void = { [weak self] buffer, _ in
            guard let captured = CapturedVoiceAudio(buffer: buffer) else { return }
            if case .dropped = continuation.yield(captured) {
                DispatchQueue.main.async {
                    guard let self else { return }
                    self.droppedAudioInputChunks += 1
                    logger.warning(
                        "voice.input_queue_drop total=\(self.droppedAudioInputChunks)"
                    )
                }
            }
        }
        inputNode.installTap(onBus: 0, bufferSize: 2048, format: inputFormat, block: tapBlock)
        self.inputTapInstalled = true

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
            self.playerTapInstalled = true
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

    func handleProcessedInput(
        _ processed: ProcessedVoiceAudio,
        engine: any VoiceSttEngine
    ) async {
        guard self.task != nil else { return }
        switch self.state {
        case .idle, .connecting, .failed:
            return
        case .listening, .capturing, .thinking, .speaking, .cancelling:
            break
        }

        for frameRms in processed.rmsFramesDbfs {
            self.publishInputLevel(frameRms: frameRms)
            self.checkBargeIn(frameRms: frameRms)
        }

        guard !self.muted else { return }
        guard self.sttStatus == .ready, self.sttLoaded else { return }
        do {
            try await engine.acceptSamples(processed.samples16k)
        } catch {
            if case FluidAudioSttError.notLoaded = error {
                logger.debug("stt.acceptSamples_notLoaded_race")
            } else {
                logger.warning(
                    "stt.acceptSamples failed: \(error.localizedDescription, privacy: .public)"
                )
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
                if self.clientTtsActive,
                   self.partialTranscript.split(whereSeparator: \.isWhitespace).count >= 3 {
                    self.cancelClientTtsTurn()
                }
                let tsMs = UInt64(Date.now.timeIntervalSince1970 * 1_000)
                logger.info("voice.barge_in_sending playedMs=\(playedMs)")
                let evidence = self.partialTranscript
                Task { [weak self] in
                    guard let self else { return }
                    await self.sendClient(.bargeIn(
                        tsMs: tsMs,
                        playbackMsPlayed: playedMs,
                        partialText: evidence.isEmpty ? nil : evidence
                    ))
                    // A MinWords rejection has no acknowledgement. Permit a
                    // later candidate once STT has accumulated more evidence.
                    try? await Task.sleep(for: .milliseconds(250))
                    guard self.state == .speaking else { return }
                    self.bargeInSent = false
                    self.bargeInCount = 0
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

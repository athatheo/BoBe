@preconcurrency import AVFoundation
import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "TtsPlayback")

/// One scheduled TTS chunk. We retain the decoded `AVAudioPCMBuffer` so
/// `truncatePlayback` can reschedule the head of a straddling buffer
/// after `playerNode.stop()` wipes the queue.
struct ScheduledTtsChunk {
    let chunkId: UInt64
    let buffer: AVAudioPCMBuffer
    /// Cumulative frame index across the current turn at which this
    /// chunk's first sample lives. `playerTime.sampleTime` is also
    /// monotonic across the engine's life — we map between the two via
    /// `sampleTimeBase` / `turnFrameBase`.
    let startFrame: AVAudioFramePosition
}

@MainActor
extension VoicePipeline {
    // MARK: - Opus decode + schedule

    func ensureDecoder() {
        if self.decoder != nil { return }
        guard let format = self.ttsOutputFormat else { return }
        do {
            self.decoder = try OpusDecoder(outputFormat: format)
        } catch {
            self.lastError = "decoder init: \(error.localizedDescription)"
            logger.error("decoder init: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Decode one incoming Opus TTS frame and append the resulting PCM
    /// buffer to the player's schedule. Called from the WS receive loop
    /// for every Binary message during `.speaking`.
    func handleAudioFrame(_ bytes: Data) {
        guard let parsed = TtsFrameHeader.parse(bytes) else {
            logger.warning("voice.malformed_tts_frame size=\(bytes.count)")
            return
        }
        self.ensureDecoder()
        guard let decoder = self.decoder, let format = self.ttsOutputFormat else { return }
        // 60ms @ 24kHz upper bound for Opus frame size.
        guard let outBuffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_440)
        else { return }
        do {
            try parsed.payload.withUnsafeBytes { raw in
                let typed = UnsafeBufferPointer(
                    start: raw.bindMemory(to: UInt8.self).baseAddress,
                    count: raw.count
                )
                try decoder.decode(typed, to: outBuffer)
            }
            // Mouth envelope is driven by the playerNode tap installed in
            // configureEngine — that samples actual playback in real time
            // over the full duration of the audio, not just the brief
            // decode burst. Sampling here would peak immediately and
            // decay before the user has heard a word.
            let chunk = ScheduledTtsChunk(
                chunkId: parsed.header.chunkId,
                buffer: outBuffer,
                startFrame: self.nextScheduleFrame
            )
            self.scheduledChunks.append(chunk)
            self.nextScheduleFrame += AVAudioFramePosition(outBuffer.frameLength)
            // Fire-and-forget queue insertion — the async overload returns
            // when the buffer FINISHES playing and deadlocks streaming inserts.
            self.playerNode.scheduleBuffer(outBuffer, completionHandler: nil)
            self.trimPlayedChunks()
        } catch {
            self.lastError = "decode: \(error.localizedDescription)"
            logger.error("decode: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Drop fully-played `scheduledChunks` from the head, keeping a short
    /// tail so barge-in truncation can still reslice. Without this, a long
    /// uninterrupted reply retains every decoded `AVAudioPCMBuffer` for
    /// the entire turn — ~40 MB for a 5-minute response.
    ///
    /// The retained tail covers ~2 seconds of recently-played audio. That's
    /// well past the server's typical barge-in detection latency (~150 ms)
    /// while leaving enough margin for any future server-driven truncation
    /// that's already in flight when this trim fires.
    func trimPlayedChunks() {
        let played = self.playedFramesThisTurn()
        let retainTailFrames = AVAudioFramePosition(self.playbackSampleRate * 2.0)
        let cutoff = played - retainTailFrames
        guard cutoff > 0 else { return }
        var dropCount = 0
        for chunk in self.scheduledChunks {
            let chunkEnd = chunk.startFrame + AVAudioFramePosition(chunk.buffer.frameLength)
            if chunkEnd <= cutoff {
                dropCount += 1
            } else {
                break
            }
        }
        if dropCount > 0 {
            self.scheduledChunks.removeFirst(dropCount)
        }
    }

    // MARK: - Per-turn sample math

    /// Frames played by `playerNode` since this turn began, accounting for
    /// `stop()` / `play()` resets via `turnFrameBase`. Returns
    /// `turnFrameBase` if the render clock hasn't tickled yet.
    func playedFramesThisTurn() -> AVAudioFramePosition {
        guard let lrt = self.playerNode.lastRenderTime,
              let pt = self.playerNode.playerTime(forNodeTime: lrt)
        else {
            return self.turnFrameBase
        }
        let st = max(self.sampleTimeBase, pt.sampleTime)
        return self.turnFrameBase + (st - self.sampleTimeBase)
    }

    func currentPlayerSampleTime() -> AVAudioFramePosition {
        guard let lrt = self.playerNode.lastRenderTime,
              let pt = self.playerNode.playerTime(forNodeTime: lrt)
        else {
            return 0
        }
        return max(0, pt.sampleTime)
    }

    // MARK: - Truncation (barge-in)

    /// Post-barge-in: keep first `keepMs` of audio, drop the tail.
    /// Capture played-frames before `stop()` (resets player clock), wipe
    /// queue, re-schedule slices of retained chunks covering [played,
    /// target), bump `turnFrameBase` so render-clock stays turn-relative.
    func truncatePlayback(keepMs: UInt64) {
        let target = AVAudioFramePosition(Double(keepMs) * self.playbackSampleRate / 1_000.0)
        let played = self.playedFramesThisTurn()

        self.playerNode.stop()
        self.bargeInCount = 0
        self.bargeInSent = false

        guard target > played else {
            // We're already past the keep point — nothing to re-schedule.
            self.scheduledChunks.removeAll(keepingCapacity: true)
            self.nextScheduleFrame = played
            self.turnFrameBase = played
            self.sampleTimeBase = self.currentPlayerSampleTime()
            self.playerNode.play()
            return
        }

        var newChunks: [ScheduledTtsChunk] = []
        for chunk in self.scheduledChunks {
            let chunkStart = chunk.startFrame
            let chunkEnd = chunkStart + AVAudioFramePosition(chunk.buffer.frameLength)
            let sliceStart = max(chunkStart, played)
            let sliceEnd = min(chunkEnd, target)
            guard sliceEnd > sliceStart else { continue }
            let offset = AVAudioFrameCount(sliceStart - chunkStart)
            let frames = AVAudioFrameCount(sliceEnd - sliceStart)
            let buffer: AVAudioPCMBuffer
            if offset == 0, frames == chunk.buffer.frameLength {
                buffer = chunk.buffer
            } else if let sliced = sliceInt16Buffer(chunk.buffer, offset: offset, frames: frames) {
                buffer = sliced
            } else {
                continue
            }
            newChunks.append(ScheduledTtsChunk(
                chunkId: chunk.chunkId,
                buffer: buffer,
                startFrame: sliceStart
            ))
        }

        // Replace (not clear) so a second truncate within the same turn
        // can slice further from the still-tracked tail.
        self.scheduledChunks = newChunks
        self.nextScheduleFrame = target
        self.turnFrameBase = played
        self.sampleTimeBase = self.currentPlayerSampleTime()

        for chunk in newChunks {
            self.playerNode.scheduleBuffer(chunk.buffer, completionHandler: nil)
        }
        self.playerNode.play()
    }

    // MARK: - Mouth envelope (TTS playback)

    /// Realtime-tap callback for the TTS playerNode. Receives RMS measured
    /// from the live audio buffer and updates `ttsOutputLevel`, which the
    /// avatar's mouth animation reads.
    ///
    /// **Why we sample audio output rather than decoded chunks**: chunks
    /// decode in microseconds but the actual audio plays out over seconds.
    /// A per-chunk envelope would crest immediately and fall silent before
    /// the user heard the first phoneme.
    ///
    /// **No state guard**: the daemon flips wire phase to `.idle` the
    /// moment it stops sending chunks, but the player still has 1-2 seconds
    /// of buffered audio that WILL play. Gating on phase silences the
    /// envelope mid-word; we keep sampling for the whole audible tail.
    func handlePlaybackRms(_ rms: Float) {
        let normalized = Self.normalizeRmsToLevel(rms)
        // Asymmetric smoothing — snap up so consonants register, glide
        // down so the mouth doesn't strobe between syllables.
        let next = normalized > self.ttsOutputLevel
            ? (self.ttsOutputLevel * 0.35) + (normalized * 0.65)
            : (self.ttsOutputLevel * 0.7) + (normalized * 0.3)
        // Suppress sub-perceptual writes so mouth rendering isn't
        // invalidated on every tap callback (~50 Hz).
        if abs(next - self.ttsOutputLevel) > 0.005 {
            self.ttsOutputLevel = next
        }
        // Update the hysteresis-latched audibility flag. This is the slow
        // signal that wide-scope views (OverlayView) read instead of
        // ttsOutputLevel itself — flips a handful of times per turn, not
        // 50 times per second.
        self.updateAudibilityLatch(level: next)
        // Record arrival so the decay loop yields to the tap. Without
        // this the decay tick multiplies the freshly-published level a
        // few ms later, producing a sawtooth.
        self.lastTtsRmsAt = Date.now.timeIntervalSince1970
    }

    /// Hysteresis: rising edge flips ON immediately; falling edge waits
    /// `audibleHangoverMs` of sustained quiet before flipping OFF. The
    /// hangover task self-cancels if the level rebounds, so a typical
    /// consonant gap doesn't trigger a needless toggle.
    private func updateAudibilityLatch(level: Float) {
        if level >= self.audibleOnThreshold {
            self.audibleHangoverTask?.cancel()
            self.audibleHangoverTask = nil
            if !self.isTtsAudible {
                self.isTtsAudible = true
            }
            return
        }
        if level < self.audibleOffThreshold, self.isTtsAudible, self.audibleHangoverTask == nil {
            // Start the trailing-silence hangover. Captured `weak self` because
            // the pipeline is a singleton but the task is conceptually scoped.
            self.audibleHangoverTask = Task { @MainActor [weak self] in
                guard let self else { return }
                try? await Task.sleep(for: .milliseconds(self.audibleHangoverMs))
                guard !Task.isCancelled else { return }
                // Re-check the latched level — if a phoneme arrived during
                // the hangover, the rising-edge handler will have cancelled
                // us. If it didn't, we drop the flag.
                if self.ttsOutputLevel < self.audibleOffThreshold {
                    self.isTtsAudible = false
                }
                self.audibleHangoverTask = nil
            }
        }
    }

    /// Decays `ttsOutputLevel` toward 0 between RMS samples. Stays
    /// alive while wire phase is `.speaking` OR the player has buffered
    /// audio past the playhead (i.e. audio that WILL play even if no
    /// more chunks arrive). Exits cleanly once both are false AND the
    /// envelope has decayed to silence.
    func startTtsLevelDecay() {
        self.ttsLevelDecayTask?.cancel()
        self.ttsLevelDecayTask = Task { @MainActor [weak self] in
            // 16ms ≈ 60Hz parity; small per-step drop so the envelope
            // flows smoothly between RMS samples. CPU is negligible.
            while let self, !Task.isCancelled {
                if self.shouldExitDecayLoop() {
                    self.ttsOutputLevel = 0
                    return
                }
                // Skip the decay multiplier while the RMS tap is
                // actively driving the envelope. Otherwise decay races
                // RMS and we see a sawtooth instead of a smooth follower.
                if Date.now.timeIntervalSince1970 - self.lastTtsRmsAt > 0.06 {
                    self.ttsOutputLevel *= 0.95
                    if self.ttsOutputLevel < 0.01 { self.ttsOutputLevel = 0 }
                }
                try? await Task.sleep(for: .milliseconds(16))
            }
        }
    }

    /// Soft stop. NO-OP while the player still has buffered audio past
    /// the playhead — the decay loop's own exit condition will fire once
    /// the audio drains. Hard-cancelling here on `applyPhase(.idle)`
    /// would rip the animation out mid-word.
    func stopTtsLevelDecay() {
        if self.audioStillPlaying() { return }
        self.ttsLevelDecayTask?.cancel()
        self.ttsLevelDecayTask = nil
        self.ttsOutputLevel = 0
        // Cancel any pending hangover and drop the latch — turn ended.
        self.audibleHangoverTask?.cancel()
        self.audibleHangoverTask = nil
        if self.isTtsAudible { self.isTtsAudible = false }
    }

    // MARK: - Mouth envelope helpers

    /// Normalize an RMS amplitude (0…1) to a perceptual 0…1 level. -60
    /// dBFS → 0, 0 dBFS → 1. Same scale as `inputLevel` so both signals
    /// read consistently on the avatar UI.
    private static func normalizeRmsToLevel(_ rms: Float) -> Float {
        let dbfs = 20 * log10(max(rms, 1e-6))
        return max(0, min(1, (dbfs + 60) / 60))
    }

    /// True when the player has buffered audio past the current playhead
    /// — i.e. audio that WILL play out even if no more chunks arrive.
    /// `internal` so the audio-device hot-swap extension can use it to
    /// defer output-device rebuilds until the current TTS turn drains.
    func audioStillPlaying() -> Bool {
        self.playedFramesThisTurn() < self.nextScheduleFrame
    }

    /// Decay loop's exit condition — wire phase is no longer `.speaking`,
    /// no buffered audio remains, and the published level has decayed.
    private func shouldExitDecayLoop() -> Bool {
        self.state != .speaking
            && !self.audioStillPlaying()
            && self.ttsOutputLevel < 0.02
    }
}

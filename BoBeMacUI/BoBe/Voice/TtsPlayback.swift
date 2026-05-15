@preconcurrency import AVFoundation
import Foundation
import Opus
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
        guard let format = AVAudioFormat(
            opusPCMFormat: .int16,
            sampleRate: self.playbackSampleRate,
            channels: 1
        ) else { return }
        do {
            self.decoder = try Opus.Decoder(format: format)
        } catch {
            self.lastError = "decoder init: \(error.localizedDescription)"
            logger.error("decoder init: \(error.localizedDescription)")
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
        guard let decoder = self.decoder else { return }
        guard let format = AVAudioFormat(
            opusPCMFormat: .int16,
            sampleRate: self.playbackSampleRate,
            channels: 1
        ),
        // 60ms @ 24kHz upper bound for Opus frame size.
        let outBuffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_440) else {
            return
        }
        do {
            try parsed.payload.withUnsafeBytes { raw in
                let typed = UnsafeBufferPointer(
                    start: raw.bindMemory(to: UInt8.self).baseAddress,
                    count: raw.count
                )
                try decoder.decode(typed, to: outBuffer)
            }
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
        } catch {
            self.lastError = "decode: \(error.localizedDescription)"
            logger.error("decode: \(error.localizedDescription)")
        }
    }

    // MARK: - Per-turn sample math

    /// Frames played by `playerNode` since this turn began, accounting for
    /// `stop()` / `play()` resets via `turnFrameBase`. Returns
    /// `turnFrameBase` if the render clock hasn't tickled yet.
    func playedFramesThisTurn() -> AVAudioFramePosition {
        guard let lrt = self.playerNode.lastRenderTime,
              let pt = self.playerNode.playerTime(forNodeTime: lrt) else {
            return self.turnFrameBase
        }
        let st = max(self.sampleTimeBase, pt.sampleTime)
        return self.turnFrameBase + (st - self.sampleTimeBase)
    }

    func currentPlayerSampleTime() -> AVAudioFramePosition {
        guard let lrt = self.playerNode.lastRenderTime,
              let pt = self.playerNode.playerTime(forNodeTime: lrt) else {
            return 0
        }
        return max(0, pt.sampleTime)
    }

    // MARK: - Truncation (barge-in)

    /// Sample-accurate playback truncation. Keeps the head of in-flight audio
    /// up to `keepMs` and drops the tail. The daemon emits `truncate{keep_ms}`
    /// after a barge-in, where `keep_ms` is what the user actually heard
    /// (max of client RMS-detect playback position and last `PlaybackAck`).
    /// Implementation:
    ///   1. Capture frames-played-this-turn BEFORE `stop()` (which resets
    ///      the player's clock).
    ///   2. `stop()` to wipe the pending queue.
    ///   3. Re-schedule slices of retained `AVAudioPCMBuffer`s covering the
    ///      half-open range [played, target). Earlier audio has already
    ///      reached the speakers; nothing to do for it. Track the slices in
    ///      `scheduledChunks` so a second truncate within the same turn
    ///      remains sample-accurate.
    ///   4. `play()` and bump `turnFrameBase` so future render-clock reads
    ///      stay in turn-relative space.
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
}

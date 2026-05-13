@preconcurrency import AVFoundation
import Foundation
import Observation
import Opus
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "VoicePipeline")

/// Production voice pipeline (client side).
///
/// The daemon owns the authoritative VAD + smart-turn + STT + TTS; this layer's
/// job is to capture mic audio, encode it to Opus, ship it to the daemon, and
/// play back the daemon's Opus reply frames. State follows the daemon's
/// `state{phase,turn_id}` messages.
///
/// Lifecycle:
///   1. `prewarm()` — once mic permission is granted; settles VPIO AGC so the
///       first second of speech isn't attenuated. Idempotent.
///   2. `toggle(daemonBaseURL:)` — connects WS, sends `hello`, starts streaming.
///       Daemon decides when speech starts/ends via Silero.
///   3. `disconnect()` — sends `control{action:abort}`, cancels WS task.
@MainActor
@Observable
public final class VoicePipeline {
    /// Mirrors the daemon's authoritative `VoicePhase` plus a client-only
    /// `.connecting` state for the WS handshake window.
    public enum State: Equatable {
        case idle
        case connecting
        case listening
        case capturing
        case thinking
        case speaking
        case cancelling
        case failed(String)
    }

    public static let shared = VoicePipeline()

    public private(set) var state: State = .idle
    /// Last surfaced error message. Today nothing in the overlay binds this;
    /// the value is also logged via `logger.error` at every write site so
    /// engineers can see it in Console.app. The Voice settings pane (#85)
    /// will surface it as a UI affordance.
    public private(set) var lastError: String?
    /// Normalized input level, 0...1, derived from the most recent 20ms
    /// frame's RMS dBFS (-60 dBFS → 0, 0 dBFS → 1). Drives the MicButton's
    /// pulsing ring so the user sees their voice is getting through before
    /// the daemon's VAD has decided whether it's a speech segment.
    public private(set) var inputLevel: Float = 0
    /// Streaming partial transcript from the daemon. Updated on every
    /// TranscriptPartial frame; cleared at turn-end. Surfaces in the
    /// MicButton tooltip + overlay so the user gets immediate feedback that
    /// "I heard you say X" rather than a silent void during STT.
    public private(set) var partialTranscript: String = ""

    /// 16kHz mono — Moonshine + Silero native. Opus 20ms frames = 320 samples.
    private let captureSampleRate: Double = 16_000
    private let captureFrameSamples: Int = 320
    /// 24kHz mono — Kokoro native.
    private let playbackSampleRate: Double = 24_000

    private let audioEngine = AVAudioEngine()
    private let playerNode = AVAudioPlayerNode()

    private var encoder: Opus.Encoder?
    private var decoder: Opus.Decoder?
    private var converter: AVAudioConverter?
    private var pcmAccumulator: [Int16] = []
    /// Reused per-frame buffer for Opus encoding to avoid a 1500B alloc on
    /// every 20ms frame. Opus VOIP @ 24kbps/20ms encodes to ~60B; the 1500B
    /// upper bound is RTP MTU-safe. The same Data is reset and reused.
    private var encodeScratch = Data(count: 1500)

    /// One scheduled TTS chunk. We retain the decoded `AVAudioPCMBuffer` so
    /// `truncatePlayback` can reschedule the head of a straddling buffer
    /// after `playerNode.stop()` wipes the queue.
    private struct ScheduledTtsChunk {
        let chunkId: UInt64
        let buffer: AVAudioPCMBuffer
        /// Cumulative frame index across the current turn at which this
        /// chunk's first sample lives. `playerTime.sampleTime` is also
        /// monotonic across the engine's life — we map between the two via
        /// `sampleTimeBase` / `turnFrameBase`.
        let startFrame: AVAudioFramePosition
    }
    private var scheduledChunks: [ScheduledTtsChunk] = []
    private var nextScheduleFrame: AVAudioFramePosition = 0
    /// Captured at the start of every `.speaking` transition. Subtracted
    /// from the player's `sampleTime` to get "frames played in this turn".
    private var sampleTimeBase: AVAudioFramePosition = 0
    /// Jumps forward by `played` on every truncate so subsequent
    /// `playedFramesThisTurn` calls stay in turn-relative space across
    /// `stop()` + `play()` (which resets the player's clock).
    private var turnFrameBase: AVAudioFramePosition = 0

    private let urlSession = URLSession(configuration: .default)
    private var task: URLSessionWebSocketTask?
    private var sessionId: String = ""
    private var keepaliveTask: Task<Void, Never>?
    /// Mirror of the daemon's 25s keepalive cadence. Pongs from our pings
    /// keep the daemon's recv-timeout window fresh; symmetric client-side
    /// pings cover the case where the daemon stalls.
    private let keepalivePingIntervalNs: UInt64 = 25_000_000_000

    private var isWarm = false

    /// Barge-in detection tuning. During `.speaking`, sustained mic energy
    /// above `bargeInRmsDbfs` for `bargeInFramesNeeded` consecutive 20ms
    /// frames (= 60ms) sends a `barge_in` message. VPIO AEC removes most of
    /// BoBe's own voice from the mic; the higher dBFS floor (vs. normal
    /// speech detection at -45) avoids residual-echo false positives.
    private let bargeInRmsDbfs: Float = -40
    private let bargeInFramesNeeded: Int = 3
    private var bargeInCount: Int = 0
    private var bargeInSent: Bool = false

    private init() {
        self.audioEngine.attach(self.playerNode)
        // Connect the player chain once with Kokoro's native format. This
        // routes TTS through the engine's mainMixer → outputNode so VPIO on
        // the output node has a reference signal for AEC.
        if let format = AVAudioFormat(
            opusPCMFormat: .int16,
            sampleRate: self.playbackSampleRate,
            channels: 1
        ) {
            self.audioEngine.connect(
                self.playerNode,
                to: self.audioEngine.mainMixerNode,
                format: format
            )
        }
    }

    // MARK: - Lifecycle

    /// Pre-warm VPIO/AGC so the first ~300ms-1s of speech isn't attenuated.
    /// Call once mic permission is granted. Idempotent.
    public func prewarm() async {
        guard !self.isWarm else { return }
        do {
            try self.configureEngine()
            self.isWarm = true
        } catch {
            self.lastError = "voice prewarm: \(error.localizedDescription)"
            logger.error("prewarm failed: \(error.localizedDescription)")
        }
    }

    public func connect(daemonBaseURL: URL) {
        self.lastError = nil

        guard let wsURL = Self.wsEndpoint(from: daemonBaseURL) else {
            self.lastError = "invalid daemon URL for WS"
            logger.error("invalid daemon URL for WS")
            return
        }

        self.state = .connecting
        let newTask = self.urlSession.webSocketTask(with: wsURL)
        self.task = newTask
        newTask.resume()
        self.receiveLoop()
        self.startKeepalive()

        self.sessionId = "voice-\(Int(Date().timeIntervalSince1970))"
        let sid = self.sessionId
        // Per-WS voice prefs from BobeStore (M4.5.0c plumbs the soul/settings
        // wiring later — for now Hello just sends nil, and the daemon falls
        // through to its defaults).
        let voiceId: String? = nil
        let speed: Float? = nil
        Task { [weak self] in
            guard let self else { return }
            await self.sendClient(.hello(
                sessionId: sid,
                captureRate: 16_000,
                playbackRate: 24_000,
                codec: "opus",
                voiceId: voiceId,
                speed: speed
            ))
        }
    }

    public func disconnect() {
        self.stopKeepalive()
        let oldTask = self.task
        self.task = nil
        self.state = .idle
        self.inputLevel = 0
        self.partialTranscript = ""
        // Send abort then cancel — sending after task=nil would drop the
        // message, and stale receiveLoop callbacks are guarded in receiveLoop
        // by the task-identity check.
        Task {
            if let oldTask {
                let abort = ClientVoiceMessage.control(action: .abort)
                if let data = try? JSONEncoder().encode(abort),
                   let text = String(data: data, encoding: .utf8) {
                    try? await oldTask.send(.string(text))
                }
                oldTask.cancel(with: .normalClosure, reason: nil)
            }
        }
        // Tear down the audio engines so the macOS mic indicator clears.
        // Without this, prewarm + first connect leaves VPIO running for the
        // app lifetime even when voice is "off".
        self.tearDownAudio()
    }

    /// Reverses `configureEngine`. Idempotent — safe to call when not warm.
    private func tearDownAudio() {
        guard self.isWarm else { return }
        self.audioEngine.inputNode.removeTap(onBus: 0)
        self.playerNode.stop()
        self.audioEngine.stop()
        self.encoder = nil
        self.converter = nil
        self.pcmAccumulator.removeAll(keepingCapacity: false)
        self.bargeInCount = 0
        self.bargeInSent = false
        self.isWarm = false
    }

    /// Single-tap toggle: idle → connect; otherwise → disconnect.
    public func toggle(daemonBaseURL: URL) async {
        switch self.state {
        case .idle, .failed:
            self.connect(daemonBaseURL: daemonBaseURL)
            var waited = 0.0
            while self.state == .connecting, waited < 2.0 {
                try? await Task.sleep(nanoseconds: 100_000_000)
                waited += 0.1
            }
            if self.state == .connecting {
                // Connection didn't make it out of .connecting in 2s. Surface
                // the failure to the UI so MicButton becomes tappable again —
                // without this transition the button stays disabled forever
                // and the user has no recourse other than restarting the app.
                let reason = self.lastError ?? "voice connection timed out"
                if self.lastError == nil {
                    self.lastError = reason
                    logger.error("voice connection timed out")
                }
                self.state = .failed(reason)
                self.disconnect()
            }
        case .connecting, .listening, .capturing, .thinking, .speaking, .cancelling:
            self.disconnect()
        }
    }

    // MARK: - Audio engine

    private func configureEngine() throws {
        let inputNode = self.audioEngine.inputNode
        try inputNode.setVoiceProcessingEnabled(true)
        // VPIO on the output node too — gives Apple's AEC a reference signal
        // for AEC during full-duplex (M4.5.5 barge-in territory). Required by
        // the architecture doc; on macOS it's idempotent if already enabled.
        try self.audioEngine.outputNode.setVoiceProcessingEnabled(true)

        let inputFormat = inputNode.inputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0 else {
            throw VoiceError.runtime("input sample rate is 0 — no mic device?")
        }
        guard let int16Format = AVAudioFormat(
            opusPCMFormat: .int16,
            sampleRate: self.captureSampleRate,
            channels: 1
        ) else {
            throw VoiceError.runtime("could not create Int16 target format")
        }
        guard let converter = AVAudioConverter(from: inputFormat, to: int16Format) else {
            throw VoiceError.runtime("could not create AVAudioConverter")
        }
        // VPIO on macOS delivers multichannel deinterleaved Float32. Channel 0
        // is the AEC'd mic; channels 1+ are reference signals. Default downmix
        // would sum them all and mask AEC. Force channel 0 only.
        if inputFormat.channelCount > 1 {
            converter.channelMap = [NSNumber(value: 0)]
        }
        self.converter = converter
        self.encoder = try Opus.Encoder(format: int16Format, application: .voip)

        // installTap's block runs on the realtime audio dispatch queue. Marked
        // @Sendable so it doesn't inherit @MainActor isolation from the
        // enclosing class — without this, Swift 6's runtime isolation check
        // crashes the realtime queue the moment audio flows.
        let tapBlock: @Sendable (AVAudioPCMBuffer, AVAudioTime) -> Void = { [weak self] buffer, _ in
            DispatchQueue.main.async {
                self?.handleInputBuffer(buffer)
            }
        }
        inputNode.installTap(onBus: 0, bufferSize: 2048, format: inputFormat, block: tapBlock)

        self.audioEngine.prepare()
        try self.audioEngine.start()
        self.playerNode.play()
        logger.info("voice engine configured; input \(self.describe(inputFormat))")
    }

    private func handleInputBuffer(_ buffer: AVAudioPCMBuffer) {
        // Stream audio to the daemon while WS is active. The daemon's Silero
        // decides what's speech; we just feed it bytes.
        guard let task = self.task,
              let converter = self.converter,
              let encoder = self.encoder else { return }
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

        final class FedState: @unchecked Sendable { var fed = false }
        let fedState = FedState()
        var err: NSError?
        let status = converter.convert(to: outBuf, error: &err) { _, status in
            if fedState.fed {
                status.pointee = .noDataNow
                return nil
            }
            fedState.fed = true
            status.pointee = .haveData
            return buffer
        }
        if status == .error || err != nil {
            self.lastError = "convert: \(err?.localizedDescription ?? "?")"
            logger.error("convert: \(err?.localizedDescription ?? "?")")
            return
        }
        guard let int16Ptr = outBuf.int16ChannelData?[0] else { return }
        let count = Int(outBuf.frameLength)
        let samples = UnsafeBufferPointer(start: int16Ptr, count: count)
        self.pcmAccumulator.append(contentsOf: samples)

        // Encode + send full 20ms frames; daemon expects continuous stream.
        // Per-frame RMS is also fed to the barge-in detector and the public
        // inputLevel observable for UI feedback (pulsing mic ring).
        while self.pcmAccumulator.count >= self.captureFrameSamples {
            let slice = Array(self.pcmAccumulator.prefix(self.captureFrameSamples))
            self.pcmAccumulator.removeFirst(self.captureFrameSamples)
            let frameRms = self.rmsDbfs(of: slice)
            self.publishInputLevel(frameRms: frameRms)
            self.checkBargeIn(frameRms: frameRms)
            self.sendEncodedFrame(samples: slice, encoder: encoder, task: task)
        }
    }

    /// Map dBFS (-60..0) → 0...1 with a soft floor so quiet background hum
    /// reads as ~0 and only deliberate speech moves the ring.
    private func publishInputLevel(frameRms: Float) {
        let floor: Float = -50
        let normalized = max(0, min(1, (frameRms - floor) / -floor))
        // Light EMA smoothing — 60Hz updates of raw RMS look jittery.
        let smoothed = (self.inputLevel * 0.6) + (normalized * 0.4)
        self.inputLevel = smoothed
    }

    private func rmsDbfs(of samples: [Int16]) -> Float {
        guard !samples.isEmpty else { return -100 }
        var sumSquares: Double = 0
        for s in samples {
            let f = Double(s) / 32_768.0
            sumSquares += f * f
        }
        let rms = sqrt(sumSquares / Double(samples.count))
        return rms > 1e-9 ? Float(20 * log10(rms)) : -100
    }

    /// Per-frame barge-in detector. Only fires during `.speaking`; resets in
    /// every other state so we don't carry stale counts between turns. Sends
    /// `barge_in` with the current playback position so the daemon can
    /// truncate the persisted assistant turn to what the user actually heard.
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
                let tsMs = UInt64(Date().timeIntervalSince1970 * 1_000)
                logger.info("voice.barge_in_sending playedMs=\(playedMs)")
                Task { [weak self] in
                    await self?.sendClient(.bargeIn(tsMs: tsMs, playbackMsPlayed: playedMs))
                }
            }
        } else {
            self.bargeInCount = 0
        }
    }

    /// Best-effort current playback position in ms, derived from the player
    /// node's render clock. ~100ms staleness is fine for truncation math.
    private func currentPlaybackMs() -> UInt64 {
        let frames = self.playedFramesThisTurn()
        let ms = Double(frames) * 1_000.0 / self.playbackSampleRate
        return UInt64(max(0, ms))
    }

    private func sendEncodedFrame(
        samples: [Int16],
        encoder: Opus.Encoder,
        task: URLSessionWebSocketTask
    ) {
        guard let buf = self.makeInt16Buffer(samples: samples) else { return }
        do {
            let n = try self.encodeScratch.withUnsafeMutableBytes { raw -> Int in
                guard let base = raw.baseAddress else { return 0 }
                let mut = UnsafeMutableRawBufferPointer(start: base, count: raw.count)
                return try encoder.encode(buf, to: mut)
            }
            // Copy out only the encoded bytes — we own the scratch and want
            // to keep reusing it on the next frame.
            let payload = Data(self.encodeScratch.prefix(n))
            task.send(.data(payload)) { error in
                if let error {
                    Task { @MainActor [weak self] in
                        self?.lastError = "ws send: \(error.localizedDescription)"
                        logger.error("ws send: \(error.localizedDescription)")
                    }
                }
            }
        } catch {
            self.lastError = "encode: \(error.localizedDescription)"
            logger.error("encode: \(error.localizedDescription)")
        }
    }

    private func makeInt16Buffer(samples: [Int16]) -> AVAudioPCMBuffer? {
        guard let format = AVAudioFormat(
            opusPCMFormat: .int16,
            sampleRate: self.captureSampleRate,
            channels: 1
        ) else { return nil }
        guard let buf = AVAudioPCMBuffer(
            pcmFormat: format,
            frameCapacity: AVAudioFrameCount(samples.count)
        ) else { return nil }
        buf.frameLength = AVAudioFrameCount(samples.count)
        guard let ch = buf.int16ChannelData?[0] else { return nil }
        samples.withUnsafeBufferPointer { src in
            guard let base = src.baseAddress else { return }
            ch.update(from: base, count: samples.count)
        }
        return buf
    }

    // MARK: - WS receive

    private func receiveLoop() {
        guard let currentTask = self.task else { return }
        currentTask.receive { [weak self, weak currentTask] result in
            Task { @MainActor [weak self] in
                guard let self else { return }
                // Stale callback from an old WS task — discard.
                guard self.task === currentTask else { return }
                switch result {
                case let .success(msg):
                    self.handleIncoming(msg)
                    self.receiveLoop()
                case let .failure(err):
                    self.lastError = "recv: \(err.localizedDescription)"
                    logger.error("recv: \(err.localizedDescription)")
                    self.state = .failed(err.localizedDescription)
                }
            }
        }
    }

    private func handleIncoming(_ msg: URLSessionWebSocketTask.Message) {
        switch msg {
        case let .string(text):
            self.handleControlJson(text)
        case let .data(bytes):
            self.handleAudioFrame(bytes)
        @unknown default:
            break
        }
    }

    private func handleControlJson(_ text: String) {
        guard let data = text.data(using: .utf8) else { return }
        let decoded: ServerVoiceMessage
        do {
            decoded = try JSONDecoder().decode(ServerVoiceMessage.self, from: data)
        } catch {
            logger.warning("malformed control JSON: \(text)")
            return
        }
        switch decoded {
        case let .state(phase, _):
            self.applyPhase(phase)
        case let .transcriptFinal(_, transcript):
            if !transcript.isEmpty {
                BobeStore.shared.appendUserVoiceMessage(transcript)
            }
        case .transcriptPartial(_, let transcript):
            // Surface the running partial so the user sees what BoBe is
            // hearing in near-real-time rather than waiting for transcript
            // final at end-of-utterance.
            self.partialTranscript = transcript
        case .ttsEnd:
            // Authoritative end-of-turn — daemon will follow with state(Listening).
            self.partialTranscript = ""
            break
        case let .truncate(_, keepMs):
            // M4.5.5 barge-in path — drop queued audio beyond keepMs.
            self.truncatePlayback(keepMs: keepMs)
        case let .error(code, message):
            self.lastError = "\(code): \(message)"
            logger.error("server: \(code): \(message)")
            // Errors during handshake should transition the mic UI to
            // .failed immediately rather than waiting for the WS close
            // to bounce us through receiveLoop's failure branch.
            if self.state == .connecting {
                self.state = .failed("\(code): \(message)")
            }
        case .unknown:
            break
        }
    }

    private func applyPhase(_ phase: VoicePhaseWire) {
        switch phase {
        case .idle: self.state = .idle
        case .listening: self.state = .listening
        case .capturing: self.state = .capturing
        case .thinking: self.state = .thinking
        case .speaking:
            // New turn — reset the per-turn schedule bookkeeping. Capture the
            // current player sampleTime as the baseline so playedFramesThisTurn
            // returns 0 right at turn start.
            if self.state != .speaking {
                self.scheduledChunks.removeAll(keepingCapacity: true)
                self.nextScheduleFrame = 0
                self.turnFrameBase = 0
                self.sampleTimeBase = self.currentPlayerSampleTime()
            }
            self.state = .speaking
            self.ensureDecoder()
        case .cancelling: self.state = .cancelling
        case .failed: self.state = .failed("daemon reported failure")
        }
    }

    /// Frames played by `playerNode` since this turn began, accounting for
    /// `stop()` / `play()` resets via `turnFrameBase`. Returns
    /// `turnFrameBase` if the render clock hasn't tickled yet.
    private func playedFramesThisTurn() -> AVAudioFramePosition {
        guard let lrt = self.playerNode.lastRenderTime,
              let pt = self.playerNode.playerTime(forNodeTime: lrt) else {
            return self.turnFrameBase
        }
        let st = max(self.sampleTimeBase, pt.sampleTime)
        return self.turnFrameBase + (st - self.sampleTimeBase)
    }

    private func currentPlayerSampleTime() -> AVAudioFramePosition {
        guard let lrt = self.playerNode.lastRenderTime,
              let pt = self.playerNode.playerTime(forNodeTime: lrt) else {
            return 0
        }
        return max(0, pt.sampleTime)
    }

    private func ensureDecoder() {
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

    private func handleAudioFrame(_ bytes: Data) {
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
    private func truncatePlayback(keepMs: UInt64) {
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

    // MARK: - Keepalive

    private func startKeepalive() {
        self.keepaliveTask?.cancel()
        self.keepaliveTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: self?.keepalivePingIntervalNs ?? 25_000_000_000)
                guard !Task.isCancelled else { return }
                guard let self, let task = self.task else { return }
                task.sendPing { error in
                    if let error {
                        logger.warning("voice keepalive ping failed: \(error.localizedDescription)")
                    }
                }
            }
        }
    }

    private func stopKeepalive() {
        self.keepaliveTask?.cancel()
        self.keepaliveTask = nil
    }

    private func sendClient(_ msg: ClientVoiceMessage) async {
        guard let task = self.task else { return }
        guard let data = try? JSONEncoder().encode(msg),
              let text = String(data: data, encoding: .utf8) else { return }
        do {
            try await task.send(.string(text))
        } catch {
            self.lastError = "ws send text: \(error.localizedDescription)"
            logger.error("ws send text: \(error.localizedDescription)")
        }
    }

    // MARK: - Helpers

    private static func wsEndpoint(from baseURL: URL) -> URL? {
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            return nil
        }
        let scheme = components.scheme?.lowercased()
        components.scheme = (scheme == "https") ? "wss" : "ws"
        components.path = "/voice/stream"
        return components.url
    }

    private func describe(_ format: AVAudioFormat) -> String {
        let fmtName: String = switch format.commonFormat {
        case .pcmFormatFloat32: "f32"
        case .pcmFormatFloat64: "f64"
        case .pcmFormatInt16: "i16"
        case .pcmFormatInt32: "i32"
        default: "?"
        }
        return "\(Int(format.sampleRate))Hz \(format.channelCount)ch \(fmtName)\(format.isInterleaved ? "" : " (planar)")"
    }
}

private enum VoiceError: Error, CustomStringConvertible {
    case runtime(String)
    var description: String {
        switch self {
        case let .runtime(s): s
        }
    }
}

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
    public private(set) var transcript: String = ""
    public private(set) var lastError: String?
    public private(set) var liveRmsDbfs: Float = -100

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

    private let urlSession = URLSession(configuration: .default)
    private var task: URLSessionWebSocketTask?
    private var sessionId: String = ""

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
        self.transcript = ""

        guard let wsURL = Self.wsEndpoint(from: daemonBaseURL) else {
            self.lastError = "invalid daemon URL for WS"
            return
        }

        self.state = .connecting
        let newTask = self.urlSession.webSocketTask(with: wsURL)
        self.task = newTask
        newTask.resume()
        self.receiveLoop()

        self.sessionId = "voice-\(Int(Date().timeIntervalSince1970))"
        let sid = self.sessionId
        Task { [weak self] in
            guard let self else { return }
            await self.sendClient(.hello(
                sessionId: sid,
                captureRate: 16_000,
                playbackRate: 24_000,
                codec: "opus"
            ))
        }
    }

    public func disconnect() {
        let oldTask = self.task
        self.task = nil
        self.state = .idle
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
            if self.state == .connecting, self.lastError == nil {
                self.lastError = "voice connection timed out"
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
            return
        }
        guard let int16Ptr = outBuf.int16ChannelData?[0] else { return }
        let count = Int(outBuf.frameLength)
        let samples = UnsafeBufferPointer(start: int16Ptr, count: count)
        self.pcmAccumulator.append(contentsOf: samples)
        self.updateRms(samples: samples)

        // Encode + send full 20ms frames; daemon expects continuous stream.
        // Per-frame RMS is also fed to the barge-in detector.
        while self.pcmAccumulator.count >= self.captureFrameSamples {
            let slice = Array(self.pcmAccumulator.prefix(self.captureFrameSamples))
            self.pcmAccumulator.removeFirst(self.captureFrameSamples)
            let frameRms = self.rmsDbfs(of: slice)
            self.checkBargeIn(frameRms: frameRms)
            self.sendEncodedFrame(samples: slice, encoder: encoder, task: task)
        }
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
        guard let lastRenderTime = self.playerNode.lastRenderTime,
              let playerTime = self.playerNode.playerTime(forNodeTime: lastRenderTime) else {
            return 0
        }
        let ms = Double(playerTime.sampleTime) * 1_000.0 / self.playbackSampleRate
        return UInt64(max(0, ms))
    }

    private func sendEncodedFrame(
        samples: [Int16],
        encoder: Opus.Encoder,
        task: URLSessionWebSocketTask
    ) {
        guard let buf = self.makeInt16Buffer(samples: samples) else { return }
        var encoded = Data(count: 1500)
        do {
            let n = try encoded.withUnsafeMutableBytes { raw -> Int in
                guard let base = raw.baseAddress else { return 0 }
                let mut = UnsafeMutableRawBufferPointer(start: base, count: raw.count)
                return try encoder.encode(buf, to: mut)
            }
            let payload = encoded.prefix(n)
            task.send(.data(Data(payload))) { error in
                if let error {
                    Task { @MainActor [weak self] in
                        self?.lastError = "ws send: \(error.localizedDescription)"
                    }
                }
            }
        } catch {
            self.lastError = "encode: \(error.localizedDescription)"
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

    private func updateRms(samples: UnsafeBufferPointer<Int16>) {
        guard !samples.isEmpty else { return }
        var sumSquares: Double = 0
        for s in samples {
            let f = Double(s) / 32_768.0
            sumSquares += f * f
        }
        let rms = sqrt(sumSquares / Double(samples.count))
        self.liveRmsDbfs = rms > 1e-9 ? Float(20 * log10(rms)) : -100
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
            self.transcript = transcript
            if !transcript.isEmpty {
                BobeStore.shared.appendUserVoiceMessage(transcript)
            }
        case .transcriptPartial:
            // Partial transcripts only emitted when streaming-STT lands later.
            break
        case .ttsEnd:
            // Authoritative end-of-turn — daemon will follow with state(Listening).
            break
        case let .truncate(_, keepMs):
            // M4.5.5 barge-in path — drop queued audio beyond keepMs.
            self.truncatePlayback(keepMs: keepMs)
        case let .error(code, message):
            self.lastError = "\(code): \(message)"
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
            self.state = .speaking
            self.ensureDecoder()
        case .cancelling: self.state = .cancelling
        case .failed: self.state = .failed("daemon reported failure")
        }
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
            // Fire-and-forget queue insertion — the async overload returns
            // when the buffer FINISHES playing and deadlocks streaming inserts.
            self.playerNode.scheduleBuffer(outBuffer, completionHandler: nil)
        } catch {
            self.lastError = "decode: \(error.localizedDescription)"
        }
    }

    /// Drop any queued audio past `keepMs` of playback. AVAudioPlayerNode's
    /// `stop()` unschedules all pending buffers; we then `reset()` to clear
    /// the timeline and `play()` to keep the node ready for the next turn.
    ///
    /// Sample-accurate truncation (keeping the first `keepMs` of in-flight
    /// audio and dropping only the tail) would require splitting the current
    /// PCM buffer at the right sample offset. For barge-in v1 we just stop
    /// everything immediately — the user's speech intent overrides whatever
    /// fragment is in flight.
    private func truncatePlayback(keepMs _: UInt64) {
        self.playerNode.stop()
        self.playerNode.reset()
        self.playerNode.play()
        // Reset barge-in latch so the user can interrupt again on the next turn.
        self.bargeInCount = 0
        self.bargeInSent = false
    }

    private func sendClient(_ msg: ClientVoiceMessage) async {
        guard let task = self.task else { return }
        guard let data = try? JSONEncoder().encode(msg),
              let text = String(data: data, encoding: .utf8) else { return }
        do {
            try await task.send(.string(text))
        } catch {
            self.lastError = "ws send text: \(error.localizedDescription)"
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

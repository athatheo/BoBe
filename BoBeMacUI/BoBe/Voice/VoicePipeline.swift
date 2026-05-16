@preconcurrency import AVFoundation
import FluidAudio
import Foundation
import Observation
import Opus
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "VoicePipeline")

/// Production voice pipeline (client side, Mode B).
///
/// The Swift client owns ASR via FluidAudio (Parakeet EOU on Apple Neural
/// Engine). This layer captures mic audio, feeds it to FluidAudio, and ships
/// the resulting transcripts (partial + final on end-of-utterance) over the
/// WS to the daemon. The daemon runs LLM + TTS only; TTS Opus frames stream
/// back and play through `AVAudioPlayerNode`. State follows the daemon's
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
    /// Last surfaced error message. Also logged via `logger.error` at every
    /// write site so engineers can see it in Console.app. `internal(set)`
    /// lets the TTS playback chain in `TtsPlayback.swift` surface decode /
    /// queue errors without needing a wrapper method.
    public internal(set) var lastError: String?
    /// Readiness of the local FluidAudio STT model. Drives UI affordances
    /// so the user sees "Downloading voice model…" instead of an
    /// unresponsive mic icon when the ~600MB Parakeet model is fetching.
    public enum SttStatus: Equatable {
        case notLoaded
        case downloading
        case ready
        case failed(String)
    }
    public private(set) var sttStatus: SttStatus = .notLoaded

    // MARK: - Readiness inputs (see VoiceReadiness.swift for the aggregator
    // + refreshDaemonState/updatePermission methods that mutate these).

    /// Mirrors `AVCaptureDevice.authorizationStatus(for: .audio)`. Owned here
    /// so `readiness` can fold it in; `MicButton` writes back via
    /// `updatePermission(_:)` after the system grant/deny dialog.
    var permission: AVAuthorizationStatus =
        AVCaptureDevice.authorizationStatus(for: .audio)

    /// Latest daemon-side install snapshot (Kokoro presence + per-model
    /// progress). `nil` until the first `refreshDaemonState()` call lands.
    var installSnapshot: VoiceInstallSnapshot?

    /// Mirror of `DaemonSettings.voiceEnabled`. `nil` until first fetch lands;
    /// `false` flips the mic button into a hidden / disabled state.
    var voiceEnabled: Bool?

    /// Mirrors of `DaemonSettings.{voiceShowPartialCaption, voicePersona,
    /// voiceSpeed}` — populated by `refreshDaemonState`. Caption drives
    /// `VoicePartialCaption` visibility; persona + speed get sent in every
    /// Hello so the daemon doesn't fall back to its config default.
    var showPartialCaption: Bool = true
    var voicePersona: String?
    var voiceSpeed: Float?

    /// Dedup guard so concurrent `refreshDaemonState()` calls share one
    /// fetch instead of stampeding the daemon at view-appear time.
    var refreshDaemonTask: Task<Void, Never>?

    /// Normalized input level, 0...1, derived from the most recent 20ms
    /// frame's RMS dBFS (-60 dBFS → 0, 0 dBFS → 1). Drives the MicButton's
    /// pulsing ring so the user sees their voice is getting through before
    /// the daemon's VAD has decided whether it's a speech segment.
    public private(set) var inputLevel: Float = 0
    /// Streaming partial transcript from the local FluidAudio Parakeet
    /// streaming ASR (Mode B). Updated on every partial callback; cleared
    /// at end-of-utterance. Surfaces in `VoicePartialCaption` and the
    /// MicButton tooltip so the user gets immediate feedback that
    /// "I heard you say X" while they speak.
    public private(set) var partialTranscript: String = ""

    /// 16kHz mono — Parakeet + Silero native. Used by the int16 converter
    /// only for RMS/barge-in detection; FluidAudio resamples internally.
    private let captureSampleRate: Double = 16_000
    /// 20ms slice at 16kHz — RMS + barge-in cadence (not a wire frame size
    /// in Mode B, since the client never sends audio).
    private let rmsFrameSamples: Int = 320
    /// 24kHz mono — Kokoro native. Internal so the TTS playback chain in
    /// `TtsPlayback.swift` can use it for buffer sizing + truncation math.
    let playbackSampleRate: Double = 24_000

    private let audioEngine = AVAudioEngine()
    // Internal so the TTS playback chain in `TtsPlayback.swift` can drive it.
    let playerNode = AVAudioPlayerNode()

    /// FluidAudio Parakeet EOU streaming ASR (English; built-in EOU).
    /// Loaded lazily on first `prewarm()` when the active language is
    /// English; reused across reconnects. (Internal so the engine-selection
    /// helpers in `VoiceReadiness.swift` can resolve it.)
    let parakeetStt = FluidAudioStt()
    /// FluidAudio Qwen3 ASR + VAD-driven EOU (Mandarin, plus future
    /// Spanish / Greek / Korean / Japanese — Qwen3 is multilingual).
    /// Loaded lazily when the active language is non-English.
    let qwen3Stt = FluidAudioQwen3Stt()
    /// Latest user-selected language from daemon settings. Refreshed by
    /// `refreshDaemonState()` so presence checks + readiness reflect the
    /// current pick. Used to resolve `activeStt` BEFORE a WS session opens.
    /// Once `connect()` runs, `sessionSttLanguage` takes over so a settings
    /// change mid-session doesn't swap the engine under live audio.
    var activeSttLanguage: String = "en"

    /// Language captured at `connect()` time and held until `disconnect()`.
    /// `activeStt` reads this when set; otherwise falls back to
    /// `activeSttLanguage`. Locks the engine to one for the WS session.
    /// (Internal so the engine-selection helpers in VoiceReadiness.swift
    /// can read it.)
    var sessionSttLanguage: String?

    /// Per-language "have I successfully called loadModels on this engine"
    /// flag. Each engine internally dedupes, but tracking here avoids the
    /// actor hop on every prewarm + lets a language switch correctly
    /// re-trigger a load for the new engine.
    private var loadedLanguages: Set<String> = []
    /// Convenience — true if the currently effective engine has been
    /// successfully loaded at least once this app run.
    private var sttLoaded: Bool {
        self.loadedLanguages.contains(self.effectiveLanguage)
    }
    /// Turn_id minted on the first partial of an utterance; reused on the
    /// EOU final and cleared after sending. Each utterance gets a fresh id.
    private var pendingTurnId: String?

    // Engine-selection helpers live in VoiceReadiness.swift since they're
    // conceptually part of the readiness layer (which model presence to
    // check, which engine to load). See `effectiveLanguage`, `activeStt`,
    // and `activeModelIsInstalled` there.

    // TTS playback state. Internal so the extension in `TtsPlayback.swift`
    // can drive scheduling + truncation. ScheduledTtsChunk + the methods
    // (handleAudioFrame, truncatePlayback, ensureDecoder, playedFramesThisTurn,
    // currentPlayerSampleTime) all live there.
    var decoder: Opus.Decoder?
    var scheduledChunks: [ScheduledTtsChunk] = []
    var nextScheduleFrame: AVAudioFramePosition = 0
    /// Captured at the start of every `.speaking` transition. Subtracted
    /// from the player's `sampleTime` to get "frames played in this turn".
    var sampleTimeBase: AVAudioFramePosition = 0
    /// Jumps forward by `played` on every truncate so subsequent
    /// `playedFramesThisTurn` calls stay in turn-relative space across
    /// `stop()` + `play()` (which resets the player's clock).
    var turnFrameBase: AVAudioFramePosition = 0

    private var converter: AVAudioConverter?
    private var pcmAccumulator: [Int16] = []

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
    // Internal so `TtsPlayback.truncatePlayback` can reset them on barge-in.
    var bargeInCount: Int = 0
    var bargeInSent: Bool = false

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
        // Auto-refresh aggregated readiness whenever upstream state likely
        // changed (wizard completed, settings/install endpoint mutated).
        // Consumers used to do this themselves with parallel observers; the
        // centralized refresh keeps the four underlying signals coherent.
        for name in [Notification.Name.bobeWelcomeCompleted, .bobeVoiceConfigChanged] {
            NotificationCenter.default.addObserver(
                forName: name,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    await self?.refreshDaemonState()
                }
            }
        }
    }

    // MARK: - Lifecycle

    /// Pre-warm VPIO/AGC so the first ~300ms-1s of speech isn't attenuated.
    /// STT model loading is now decoupled — call `bootstrapSttPresence()`
    /// once at boot to set readiness based on filesystem, then
    /// `ensureSttLoaded()` lazily in the background. Idempotent.
    public func prewarm() async {
        // If the model is on disk, set ready immediately so the mic isn't
        // gated on FluidAudio's ANE warm-up. The actor's `loadModels`
        // still has to run before the first audio buffer reaches it — kick
        // it off in the background, but don't block.
        self.bootstrapSttPresence()
        if self.sttStatus != .ready {
            // No presence yet — caller (wizard or settings) needs to drive
            // an explicit `ensureSttLoaded()`. Don't auto-download here.
        } else if !self.sttLoaded {
            Task { [weak self] in await self?.ensureSttLoaded() }
        }
        guard !self.isWarm else { return }
        do {
            try self.configureEngine()
            self.isWarm = true
        } catch {
            self.lastError = "voice prewarm: \(error.localizedDescription)"
            logger.error("prewarm failed: \(error.localizedDescription)")
        }
    }

    /// One-shot filesystem presence probe. Cheap (3 syscalls) — sets
    /// `sttStatus = .ready` instantly if Parakeet is already cached so the
    /// mic icon isn't spinner-gated through every launch. The actor's real
    /// `loadModels` still needs to run before the first audio buffer; that
    /// happens lazily after this returns.
    public func bootstrapSttPresence() {
        if self.sttStatus == .ready { return }
        self.sttStatus = self.activeModelIsInstalled ? .ready : .notLoaded
    }

    /// Lazy-load the FluidAudio model matching the active language. First
    /// run downloads from HuggingFace (~600MB Parakeet for English, ~1.75GB
    /// Qwen3 for Mandarin); subsequent runs are near-instant from the local
    /// cache. Concurrent callers all await the same load (deduped inside
    /// each engine). Bounded by a 120s timeout so a stalled network can't
    /// hang the UI indefinitely — the Qwen3 budget is doubled vs. Parakeet
    /// to fit the larger bundle.
    public func ensureSttLoaded() async {
        if self.sttLoaded { return }
        // Only flip to `.downloading` if we're not already presence-confirmed.
        // A presence-confirmed cache hit should stay visually `.ready` while
        // the actor warms in the background.
        if self.sttStatus != .ready && self.sttStatus != .downloading {
            self.sttStatus = .downloading
        }
        let onPartial: @Sendable (String) -> Void = { [weak self] text in
            Task { @MainActor in self?.handleSttPartial(text) }
        }
        let onEou: @Sendable (String) -> Void = { [weak self] text in
            Task { @MainActor in self?.handleSttEou(text) }
        }
        let engine = self.activeStt
        let language = self.activeSttLanguage
        let timeoutSeconds: TimeInterval = (language == "en") ? 60 : 120
        do {
            try await withVoiceLoadTimeout(seconds: timeoutSeconds) {
                try await engine.loadModels(onPartial: onPartial, onEou: onEou)
            }
            self.loadedLanguages.insert(language)
            self.sttStatus = .ready
            logger.info("FluidAudio STT loaded (language=\(language))")
        } catch is VoiceLoadTimeout {
            let msg = "STT load timed out after \(Int(timeoutSeconds))s"
            self.lastError = msg
            self.sttStatus = .failed(msg)
            logger.error("FluidAudio STT load timed out (language=\(language))")
        } catch {
            let msg = error.localizedDescription
            self.lastError = "STT load: \(msg)"
            self.sttStatus = .failed(msg)
            logger.error("FluidAudio STT load failed (language=\(language)): \(msg)")
        }
    }

    /// Streaming partial from FluidAudio. Mint a turn_id on first partial of
    /// the utterance, update the local caption, ship to daemon for
    /// cancel-phrase + MinWords gating.
    private func handleSttPartial(_ text: String) {
        guard !text.isEmpty else { return }
        if self.pendingTurnId == nil {
            self.pendingTurnId = "voice_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
        }
        let turnId = self.pendingTurnId ?? ""
        self.partialTranscript = text
        Task { [weak self] in
            await self?.sendClient(.transcriptPartial(turnId: turnId, text: text))
        }
    }

    /// End-of-utterance from FluidAudio. Ship the final transcript to the
    /// daemon (which admits the turn + runs LLM/TTS), reset the engine for
    /// the next utterance.
    private func handleSttEou(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        // Guard against EOU timers firing after disconnect tore down the
        // WS. The onEou closure trampolines onto MainActor; by the time
        // we run here disconnect() may have set self.task = nil. Without
        // this gate the closure would generate a fresh turn_id and try
        // to push transcript_final on a closed task.
        guard self.task != nil else {
            logger.debug("voice.eou_after_disconnect_ignored")
            return
        }
        let turnId = self.pendingTurnId ?? "voice_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
        self.pendingTurnId = nil
        self.partialTranscript = ""
        let engine = self.activeStt
        Task { [weak self] in
            guard let self else { return }
            await self.sendClient(.transcriptFinal(turnId: turnId, text: trimmed))
            // Clear the engine's buffer so the next utterance starts fresh.
            try? await engine.reset()
        }
    }

    public func connect(daemonBaseURL: URL) {
        self.lastError = nil

        guard let wsURL = voiceWsEndpoint(from: daemonBaseURL) else {
            self.lastError = "invalid daemon URL for WS"
            logger.error("invalid daemon URL for WS")
            return
        }

        // Lock the engine to the user's chosen language for the lifetime of
        // this WS session. Settings can change after connect; we won't swap
        // the engine under live audio.
        self.sessionSttLanguage = self.activeSttLanguage
        let language = self.activeSttLanguage

        self.state = .connecting
        let newTask = self.urlSession.webSocketTask(with: wsURL)
        self.task = newTask
        newTask.resume()
        self.receiveLoop()
        self.startKeepalive()

        self.sessionId = "voice-\(Int(Date().timeIntervalSince1970))"
        let sid = self.sessionId
        // Per-WS voice prefs sourced from the daemon settings mirror that
        // `refreshDaemonState` keeps current. Sending them explicitly in
        // Hello means the daemon doesn't have to fall back to its config
        // default — the user's pick flows through to this exact session.
        let voiceId = self.voicePersona
        let speed = self.voiceSpeed
        Task { [weak self] in
            guard let self else { return }
            // After a previous disconnect(), tearDownAudio() removed the input
            // tap and set isWarm=false. prewarm() is the only place the engine
            // is configured, and it's idempotent — call it here so the second
            // (and Nth) connect() rebuilds the audio path. Without this, a
            // close→reopen leaves the WS connected but no mic frames flowing.
            await self.prewarm()
            await self.sendClient(.hello(
                sessionId: sid,
                playbackRate: 24_000,
                voiceId: voiceId,
                speed: speed,
                language: language
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
        // Unlock the session-language so the next connect re-reads from
        // settings; a between-sessions language change picks up here.
        self.sessionSttLanguage = nil
    }

    /// Reverses `configureEngine`. Idempotent — safe to call when not warm.
    private func tearDownAudio() {
        guard self.isWarm else { return }
        self.audioEngine.inputNode.removeTap(onBus: 0)
        self.playerNode.stop()
        self.audioEngine.stop()
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

    /// Send a click-driven barge-in. Used by the overlay's stop button so
    /// the user can interrupt BoBe mid-reply without speaking over them.
    /// No-op when the daemon is idle / listening — there's nothing to stop.
    public func interrupt() {
        guard let task = self.task else { return }
        switch self.state {
        case .thinking, .speaking, .capturing:
            break
        default:
            return
        }
        let abort = ClientVoiceMessage.control(action: .abort)
        guard let data = try? JSONEncoder().encode(abort),
              let text = String(data: data, encoding: .utf8) else { return }
        Task { [weak self, weak task] in
            try? await task?.send(.string(text))
            await MainActor.run {
                self?.state = .cancelling
            }
        }
    }

    // MARK: - Audio engine

    private func configureEngine() throws {
        let inputNode = self.audioEngine.inputNode
        try inputNode.setVoiceProcessingEnabled(true)
        // VPIO on the output node too — gives Apple's AEC a reference signal
        // for AEC during full-duplex (barge-in path). Required by
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
        // No Opus encoder in Mode B — client never sends audio over WS.

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
        logger.info("voice engine configured; input \(describe(inputFormat))")
    }

    private func handleInputBuffer(_ buffer: AVAudioPCMBuffer) {
        // Mode B: feed audio to local FluidAudio STT, not the daemon. The
        // daemon receives transcripts, not audio.
        guard self.task != nil,
              let converter = self.converter else { return }
        switch self.state {
        case .idle, .connecting, .failed:
            return
        case .listening, .capturing, .thinking, .speaking, .cancelling:
            break
        }

        // RMS + barge-in path still uses int16 16k mono — same converter
        // pipeline as before for the inputLevel ring and the RMS-detected
        // barge-in during TTS playback.
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

        while self.pcmAccumulator.count >= self.rmsFrameSamples {
            let slice = Array(self.pcmAccumulator.prefix(self.rmsFrameSamples))
            self.pcmAccumulator.removeFirst(self.rmsFrameSamples)
            let frameRms = self.rmsDbfs(of: slice)
            self.publishInputLevel(frameRms: frameRms)
            self.checkBargeIn(frameRms: frameRms)
        }

        // Feed the ORIGINAL Float32 buffer to FluidAudio — each engine
        // resamples internally (Parakeet) or via its own AVAudioConverter
        // (Qwen3) so we don't need to pre-convert. Mute gates the feed too
        // so we don't transcribe during user-requested silence.
        guard !self.muted else { return }
        let engine = self.activeStt
        Task { [buffer] in
            do {
                try await engine.acceptAudio(buffer)
            } catch {
                // FluidAudio errors during a streaming pass are non-fatal —
                // most likely a transient model state issue. Log and keep
                // capturing; the next chunk's processBufferedAudio will
                // recover.
                logger.warning("stt.acceptAudio failed: \(error.localizedDescription)")
            }
        }
    }

    /// Client-requested mute — gates FluidAudio feeding so we don't
    /// transcribe during silence the user is enforcing. Toggled by the
    /// daemon's mute/unmute Control messages echoed locally.
    private var muted: Bool = false

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
        case let .helloAck(voicePack, _):
            logger.info("voice.hello_ack voicePack=\(voicePack)")
        case let .state(phase, _):
            self.applyPhase(phase)
        case let .transcriptFinal(_, transcript):
            if !transcript.isEmpty {
                BobeStore.shared.appendUserVoiceMessage(transcript)
            }
        case .ttsEnd:
            // Authoritative end-of-turn — daemon will follow with state(Listening).
            self.partialTranscript = ""
        case let .truncate(_, keepMs):
            // Barge-in path — drop queued audio beyond keepMs.
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
        }
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

}

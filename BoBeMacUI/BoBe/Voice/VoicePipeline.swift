@preconcurrency import AudioToolbox
@preconcurrency import AVFoundation
import FluidAudio
import Foundation
import Observation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "VoicePipeline")

/// Mode B voice pipeline: client owns ASR (FluidAudio on ANE), ships
/// transcripts over WS, plays back daemon TTS via AVAudioPlayerNode.
/// Lifecycle: `prewarm()` (settles VPIO AGC) → `toggle()` (connects WS) →
/// `disconnect()` (sends abort, cancels task). State mirrors daemon `state`.
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

    /// `internal(set)` so the `+WebSocket` extension can drive phase
    /// transitions via `applyPhase(_:)`. External callers (views) remain
    /// read-only.
    public internal(set) var state: State = .idle
    /// Last surfaced error message. Logged via `logger.error` at write sites.
    /// `internal(set)` lets `TtsPlayback.swift` surface decode/queue errors.
    public internal(set) var lastError: String?
    /// Readiness of the local FluidAudio STT model. Surfaces in UI so the
    /// user sees "Downloading…" instead of an unresponsive mic.
    public enum SttStatus: Equatable {
        case notLoaded
        case downloading
        case ready
        case failed(String)
    }

    public internal(set) var sttStatus: SttStatus = .notLoaded
    private(set) var sttLoadProgress: VoiceSttLoadProgress?
    private(set) var sttDownloadLanguage: String?

    // MARK: - Readiness inputs (see VoiceReadiness.swift for the aggregator

    // + refreshDaemonState/updatePermission methods that mutate these).

    /// Mirrors `AVCaptureDevice.authorizationStatus(for: .audio)`. Owned here
    /// so `readiness` can fold it in; `MicButton` writes back via
    /// `updatePermission(_:)`.
    var permission: AVAuthorizationStatus =
        AVCaptureDevice.authorizationStatus(for: .audio)

    /// Latest daemon-side install snapshot. `nil` until first refresh.
    var installSnapshot: VoiceInstallSnapshot?

    /// Mirror of `DaemonSettings.voiceEnabled`. `nil` until first fetch; `false`
    /// hides the mic button.
    var voiceEnabled: Bool?

    /// Mirrors of `DaemonSettings.{voiceShowPartialCaption, voicePersona,
    /// voiceSpeed}`. Persona + speed are sent in every Hello so the daemon
    /// doesn't fall back to its config default.
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
    /// `internal(set)` so `+Audio.publishInputLevel` can publish smoothed
    /// values. External callers remain read-only.
    public internal(set) var inputLevel: Float = 0
    /// Normalized TTS output level (0...1), driven by TtsPlayback. Drives
    /// the avatar's mouth in `SpeakingEyes`. Decays via `ttsLevelDecayTask`.
    ///
    /// **Don't read this for structural state decisions.** It mutates at ~50 Hz
    /// during speech, which causes any observer in a wide-scope view (e.g.
    /// `OverlayView.body`) to re-evaluate at the same cadence. Use
    /// `isTtsAudible` instead — that is a hysteresis-latched boolean that
    /// only flips a few times per turn.
    public internal(set) var ttsOutputLevel: Float = 0
    /// Hysteresis-latched audibility flag. Flips ON when `ttsOutputLevel`
    /// climbs over `audibleOnThreshold` for the first time, flips OFF only
    /// after it stays below `audibleOffThreshold` for `audibleHangoverMs`.
    /// Designed so that views reading this signal (avatar state, floating-
    /// bubble dismiss timer) re-evaluate a handful of times per turn rather
    /// than at the RMS sample rate.
    ///
    /// `internal(set)` because the latching logic lives in `TtsPlayback`'s
    /// extension; only the pipeline file should be writing this.
    public internal(set) var isTtsAudible: Bool = false

    /// Audibility hysteresis tuning. Asymmetric on purpose: rise quickly
    /// (so the avatar reacts to the first phoneme), fall slowly so consonants
    /// dipping briefly below the rise threshold don't flap the latch.
    /// `internal` so the extension in `TtsPlayback` can read them.
    let audibleOnThreshold: Float = 0.04
    let audibleOffThreshold: Float = 0.01
    /// Trailing-silence window the level must stay under `audibleOffThreshold`
    /// for before `isTtsAudible` flips false. Picked to roughly bridge the
    /// longest consonant gap in normal speech (~250-400 ms).
    let audibleHangoverMs: Int = 350
    /// Started fresh each time RMS drops below the off threshold.
    /// `internal` so `TtsPlayback.updateAudibilityLatch` + `stopTtsLevelDecay`
    /// can manage its lifetime.
    var audibleHangoverTask: Task<Void, Never>?

    /// Streaming partial transcript from FluidAudio (Mode B). Cleared at
    /// end-of-utterance. Surfaces in `VoicePartialCaption` + MicButton tooltip.
    /// `internal(set)` so the `+WebSocket` extension can clear it on
    /// `tts_end`. External callers remain read-only.
    public internal(set) var partialTranscript: String = ""

    /// 24kHz mono — Kokoro native. Internal so the TTS playback chain in
    /// `TtsPlayback.swift` can use it for buffer sizing + truncation math.
    let playbackSampleRate: Double = Double(VoiceWire.ttsOutputSampleRate)

    /// Reusable PCM format for Kokoro TTS output. Constructed once in `init`
    /// and shared across engine.connect, decoder init, and per-frame buffer
    /// alloc — the allocator was hot at ~50 Hz during TTS.
    let ttsOutputFormat: AVAudioFormat?

    /// Shared JSON encoder/decoder for WS control messages. `sendClient` fires
    /// on every transcript partial (~20-80/sec) — fresh encoders per call
    /// were measurable in instruments.
    static let jsonEncoder = JSONEncoder()
    static let jsonDecoder = JSONDecoder()

    /// `internal` for the `+Audio` extension's engine configure / teardown.
    let audioEngine = AVAudioEngine()
    /// Internal so the TTS playback chain in `TtsPlayback.swift` can drive it.
    let playerNode = AVAudioPlayerNode()

    /// FluidAudio Parakeet streaming ASR (English; built-in EOU). Loaded
    /// lazily on first `prewarm()` for English.
    let parakeetStt = FluidAudioStt()
    /// FluidAudio Nemotron streaming ASR + VAD. Loaded lazily for non-English.
    let nemotronStt = FluidAudioNemotronStt()
    /// Latest user-selected language from daemon settings. Used to resolve
    /// `activeStt` before a WS session opens; `sessionSttLanguage` locks for
    /// the session after `connect()`.
    var activeSttLanguage: String = "en"

    /// Language captured at `connect()` time and held until `disconnect()`,
    /// so a mid-session settings change doesn't swap engines under live audio.
    var sessionSttLanguage: String?
    var sessionTtsBackend: VoiceTtsBackend?
    var sessionVoicePersona: String?
    var sessionVoiceSpeed: Float?

    /// Per-language "have I successfully called loadModels on this engine"
    /// flag. Each engine internally dedupes, but tracking here avoids the
    /// actor hop on every prewarm + lets a language switch correctly
    /// re-trigger a load for the new engine.
    var loadedLanguages: Set<String> = []
    /// `internal` so the `+Audio` extension can gate FluidAudio feeding
    /// on STT model readiness.
    var sttLoaded: Bool {
        self.loadedLanguages.contains(self.effectiveLanguage)
    }

    /// Turn_id minted on first partial; reused on EOU; cleared after send.
    /// `internal` so `+Audio.tearDownAudio` can clear it on engine rebuild.
    var pendingTurnId: String?
    var pendingPartialForServer: String?
    var partialSendTask: Task<Void, Never>?

    // Engine helpers (`effectiveLanguage`, `activeStt`, `activeModelIsInstalled`)
    // live in VoiceReadiness.swift.

    // TTS playback state. Internal so the extension in `TtsPlayback.swift`
    // can drive scheduling + truncation. ScheduledTtsChunk + the methods
    // (handleAudioFrame, truncatePlayback, ensureDecoder, playedFramesThisTurn,
    // currentPlayerSampleTime) all live there.
    var decoder: OpusDecoder?
    var scheduledChunks: [ScheduledTtsChunk] = []
    var nextScheduleFrame: AVAudioFramePosition = 0
    /// Captured at the start of every `.speaking` transition. Subtracted
    /// from the player's `sampleTime` to get "frames played in this turn".
    var sampleTimeBase: AVAudioFramePosition = 0
    /// Jumps forward by `played` on every truncate so subsequent
    /// `playedFramesThisTurn` calls stay in turn-relative space across
    /// `stop()` + `play()` (which resets the player's clock).
    var turnFrameBase: AVAudioFramePosition = 0

    let inputProcessor = VoiceInputProcessor()
    var audioInputContinuation: AsyncStream<CapturedVoiceAudio>.Continuation?
    var audioInputTask: Task<Void, Never>?
    var droppedAudioInputChunks = 0
    let clientTtsEngine = ClientTtsEngine()
    var clientTtsQueue: ClientTtsSentenceQueue?
    var clientTtsTask: Task<Void, Never>?
    var clientTtsActive = false
    var clientTtsFirstAudioSent = false
    var pendingServerListening = false
    /// Client-requested mute — gates FluidAudio feeding so we don't
    /// transcribe during silence the user is enforcing. Toggled by the
    /// daemon's mute/unmute Control messages echoed locally.
    /// `internal` so the `+Audio` extension's input handler can read it.
    var muted: Bool = false

    private let urlSession = URLSession(configuration: .default)
    /// `internal` so the WebSocket extension can read/write through it.
    var task: URLSessionWebSocketTask?
    private var sessionId: String = ""
    /// `internal` so the WebSocket extension owns its lifecycle.
    var keepaliveTask: Task<Void, Never>?
    /// Decay loop for `ttsOutputLevel` between decoded chunks; owned at
    /// pipeline scope so disconnect/teardown cancels it. See TtsPlayback.
    var ttsLevelDecayTask: Task<Void, Never>?
    /// Monotonic timestamp (CACurrentMediaTime / Date.timeIntervalSince1970)
    /// of the last RMS sample handed in by the playback tap. The decay
    /// loop reads this to skip its tick when the tap is actively driving
    /// the envelope — otherwise decay would fight RMS at every frame and
    /// produce a visible sawtooth in the mouth animation.
    var lastTtsRmsAt: TimeInterval = 0
    /// Retained `NSObjectProtocol` tokens for the NotificationCenter
    /// observers added at init time. NotificationCenter holds its own
    /// strong refs to the closures, so failing to retain these doesn't
    /// silently disable the observer — but it makes `removeObserver` and
    /// any future reinstantiation safe. Singleton lifetime today makes
    /// this defensive rather than load-bearing.
    private var notificationObservers: [NSObjectProtocol] = []
    /// Mirror of the daemon's 25s keepalive cadence. Pongs from our pings
    /// keep the daemon's recv-timeout window fresh; symmetric client-side
    /// pings cover the case where the daemon stalls.
    /// `internal` so the WebSocket extension's keepalive loop can read it.
    let keepalivePingIntervalNs: UInt64 = 25_000_000_000

    /// `internal` so the audio-device hot-swap extension can re-set this
    /// after a teardown/rebuild cycle.
    var isWarm = false
    var inputVoiceProcessingEnabled = false
    var outputVoiceProcessingEnabled = false
    var inputTapInstalled = false
    var playerTapInstalled = false

    /// Tracks whether we've registered the CoreAudio default-input listener.
    /// One-shot for the process lifetime — the singleton pipeline never
    /// tears this down. `internal` so the dedicated audio-device extension
    /// can read it (kept here so isWarm + this flag live next to each other).
    var inputDeviceListenerRegistered = false
    /// Mirror flag for the default-OUTPUT device listener.
    var outputDeviceListenerRegistered = false
    /// Debounce + dedup for the device-change reconfigure. Multiple HAL
    /// callbacks fire per plug/unplug event (driver init + format change +
    /// stream-restart). We coalesce them into a single rebuild.
    var deviceChangeReconfigureTask: Task<Void, Never>?
    /// `true` when an audio-device change arrived during an active response.
    /// Rebuild waits for both wire speech and buffered playback to finish.
    var pendingAudioDeviceRebuild = false

    /// Barge-in detection tuning. During `.speaking`, sustained mic energy
    /// above `bargeInRmsDbfs` for `bargeInFramesNeeded` consecutive 20ms
    /// frames (= 60ms) sends a `barge_in` message. VPIO AEC removes most of
    /// BoBe's own voice from the mic; the higher dBFS floor (vs. normal
    /// speech detection at -45) avoids residual-echo false positives.
    /// `internal` — used by `+Audio.checkBargeIn`.
    let bargeInRmsDbfs: Float = -40
    let bargeInFramesNeeded: Int = 3
    // Internal so `TtsPlayback.truncatePlayback` can reset them on barge-in.
    var bargeInCount: Int = 0
    var bargeInSent: Bool = false

    private init() {
        self.ttsOutputFormat = AVAudioFormat(
            commonFormat: .pcmFormatInt16,
            sampleRate: self.playbackSampleRate,
            channels: 1,
            interleaved: false
        )
        self.audioEngine.attach(self.playerNode)
        // Kokoro native format → mainMixer → outputNode; VPIO needs the
        // reference signal at the output node for AEC.
        if let format = self.ttsOutputFormat {
            self.audioEngine.connect(
                self.playerNode,
                to: self.audioEngine.mainMixerNode,
                format: format
            )
        }
        // Wizard completion is a genuine cross-module broadcast (SetupWindow
        // → pipeline + BoBeApp.handleWelcomeCompleted both listen). Voice
        // settings mutations go direct via `refreshDaemonState()` from the
        // VoicePanel call sites — no NC round-trip needed there.
        let token = NotificationCenter.default.addObserver(
            forName: .bobeWelcomeCompleted,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                await self?.refreshDaemonState()
            }
        }
        self.notificationObservers.append(token)
    }

    // MARK: - Lifecycle

    /// Pre-warm VPIO/AGC so the first ~300ms-1s of speech isn't attenuated.
    /// Also kicks off STT model loading. Called from `connect()` — i.e.
    /// the user has explicitly toggled the mic on, so activating
    /// `setVoiceProcessingEnabled(true)` (which triggers macOS's
    /// system-wide audio ducking via VPIO) is appropriate.
    ///
    /// **Not** called eagerly at overlay-appear anymore — see
    /// `prewarmSttOnly()`. Eager VPIO startup ducked all other audio for
    /// the entire app lifetime, which the user reasonably interpreted as
    /// "BoBe is hijacking my speakers." Idempotent.
    public func prewarm() async -> Bool {
        // If the model is on disk, set ready immediately so the mic isn't
        // gated on FluidAudio's ANE warm-up. The actor's `loadModels`
        // still has to run before the first audio buffer reaches it — kick
        // it off in the background, but don't block.
        self.bootstrapSttPresence()
        // When the model isn't on disk we never auto-download here —
        // wizard/settings drive an explicit ensureSttLoaded().
        if self.sttStatus == .ready, !self.sttLoaded {
            Task { [weak self] in await self?.ensureSttLoaded() }
        }
        guard !self.isWarm else { return true }
        do {
            try self.configureEngine()
            self.isWarm = true
            return true
        } catch {
            self.lastError = "voice prewarm: \(error.localizedDescription)"
            logger.error("prewarm failed: \(error.localizedDescription, privacy: .public)")
            return false
        }
    }

    /// STT-only warmup. Loads the speech model into memory so the first
    /// audio frame after `toggle()` has STT ready, but **does NOT** start
    /// the audio engine.
    ///
    /// Why this matters: `configureEngine()` calls
    /// `setVoiceProcessingEnabled(true)` on the input + output nodes,
    /// which activates macOS's VPIO AudioUnit. VPIO automatically ducks
    /// every other app's audio system-wide for as long as the engine is
    /// running — and BoBe's engine used to be eagerly prewarmed on every
    /// overlay appear, so other apps stayed quiet until BoBe was quit.
    /// Now we keep VPIO off until the user actually toggles the mic.
    /// First-phoneme attenuation (~300ms-1s) is the cost; getting the
    /// user's music back is the benefit.
    public func prewarmSttOnly() async {
        self.bootstrapSttPresence()
        if self.sttStatus == .ready, !self.sttLoaded {
            Task { [weak self] in await self?.ensureSttLoaded() }
        }
    }

    /// One-shot filesystem presence probe. Cheap (3 syscalls) — sets
    /// `sttStatus = .ready` instantly if Parakeet is already cached so the
    /// mic icon isn't spinner-gated through every launch. The actor's real
    /// `loadModels` still needs to run before the first audio buffer; that
    /// happens lazily after this returns.
    public func bootstrapSttPresence() {
        if self.sttStatus == .ready {
            return
        }
        self.sttStatus = self.activeModelIsInstalled ? .ready : .notLoaded
    }

    /// Lazy-load the FluidAudio model matching the active language. First
    /// run downloads from HuggingFace; subsequent runs are near-instant from
    /// the validated ModelHub cache. Callers may request an inactive-language
    /// model from Settings without changing the current voice preference.
    public func ensureSttLoaded(for requestedLanguage: String? = nil) async {
        let language = requestedLanguage ?? self.activeSttLanguage
        if self.loadedLanguages.contains(language) {
            return
        }
        let isActiveLanguage = language == self.activeSttLanguage
        // Only flip to `.downloading` if we're not already presence-confirmed.
        // A presence-confirmed cache hit should stay visually `.ready` while
        // the actor warms in the background.
        if isActiveLanguage, self.sttStatus != .ready, self.sttStatus != .downloading {
            self.sttStatus = .downloading
        }
        self.sttDownloadLanguage = language
        self.sttLoadProgress = VoiceSttLoadProgress(
            fractionCompleted: 0,
            phase: .listing
        )
        let onPartial: @Sendable (String) async -> Void = { [weak self] text in
            await self?.handleSttPartial(text)
        }
        let onEou: @Sendable (String) async -> Void = { [weak self] text in
            await self?.handleSttEou(text)
        }
        let onProgress: @Sendable (VoiceSttLoadProgress) -> Void = { [weak self] progress in
            Task { @MainActor in
                guard self?.sttDownloadLanguage == language else { return }
                self?.sttLoadProgress = progress
            }
        }
        let engine = self.sttEngine(for: language)
        let timeoutSeconds: TimeInterval = 30 * 60
        do {
            try await withVoiceLoadTimeout(seconds: timeoutSeconds) {
                try await engine.loadModels(
                    onPartial: onPartial,
                    onEou: onEou,
                    onProgress: onProgress
                )
            }
            self.loadedLanguages.insert(language)
            if isActiveLanguage {
                self.sttStatus = .ready
            }
            if self.sttDownloadLanguage == language {
                self.sttDownloadLanguage = nil
                self.sttLoadProgress = nil
            }
            logger.info("FluidAudio STT loaded (language=\(language, privacy: .public))")
        } catch is VoiceLoadTimeout {
            await engine.cancelLoading()
            let msg = "STT load timed out after \(Int(timeoutSeconds))s"
            self.lastError = msg
            if isActiveLanguage {
                self.sttStatus = .failed(msg)
            }
            self.sttDownloadLanguage = nil
            self.sttLoadProgress = nil
            logger.error("FluidAudio STT load timed out (language=\(language, privacy: .public))")
        } catch is CancellationError {
            if isActiveLanguage {
                self.sttStatus = self.modelIsInstalled(for: language) ? .ready : .notLoaded
            }
            self.sttDownloadLanguage = nil
            self.sttLoadProgress = nil
        } catch {
            let msg = error.localizedDescription
            self.lastError = "STT load: \(msg)"
            if isActiveLanguage {
                self.sttStatus = .failed(msg)
            }
            self.sttDownloadLanguage = nil
            self.sttLoadProgress = nil
            logger.error("FluidAudio STT load failed (language=\(language, privacy: .public)): \(msg, privacy: .public)")
        }
    }

    public func prepareSelectedTts() async throws {
        switch VoiceTtsPreference.shared.backend {
        case .serverKokoro:
            try await DaemonClient.shared.startVoiceInstall()
        case .clientSupertonic:
            try await self.clientTtsEngine.prepare()
        }
    }

    public func cancelSttLoading() async {
        let language = self.sttDownloadLanguage ?? self.activeSttLanguage
        await self.sttEngine(for: language).cancelLoading()
        self.sttDownloadLanguage = nil
        self.sttLoadProgress = nil
        if language == self.activeSttLanguage {
            self.sttStatus = self.modelIsInstalled(for: language) ? .ready : .notLoaded
        }
    }

    public func unloadSttModel(for language: String) async {
        await self.sttEngine(for: language).cleanup()
        if language == "en" {
            self.loadedLanguages.remove("en")
        } else {
            self.loadedLanguages = self.loadedLanguages.filter { $0 == "en" }
        }
        if self.activeSttLanguage == language
            || (language != "en" && self.activeSttLanguage != "en") {
            self.sttStatus = .notLoaded
        }
    }

    /// Generate a hyphen-stripped lowercase UUID-backed turn id. Shape
    /// expected by the daemon for transcript_partial / transcript_final.
    private static func mintTurnId() -> String {
        "voice_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
    }

    /// Daemon needs the partial for cancel-phrase + MinWords gating.
    private func handleSttPartial(_ text: String) {
        guard !text.isEmpty else { return }
        if self.pendingTurnId == nil {
            self.pendingTurnId = Self.mintTurnId()
        }
        let turnId = self.pendingTurnId ?? ""
        self.partialTranscript = text
        self.pendingPartialForServer = text
        guard self.partialSendTask == nil else { return }
        self.partialSendTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(80))
            guard let self, !Task.isCancelled else { return }
            let latest = self.pendingPartialForServer
            self.pendingPartialForServer = nil
            self.partialSendTask = nil
            if let latest {
                await self.sendClient(.transcriptPartial(turnId: turnId, text: latest))
            }
        }
    }

    private func handleSttEou(_ text: String) async {
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
        let turnId = self.pendingTurnId ?? Self.mintTurnId()
        self.pendingTurnId = nil
        self.partialSendTask?.cancel()
        self.partialSendTask = nil
        self.pendingPartialForServer = nil
        self.partialTranscript = ""
        await self.sendClient(.transcriptFinal(turnId: turnId, text: trimmed))
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
        self.sessionTtsBackend = VoiceTtsPreference.shared.backend
        self.sessionVoicePersona = self.voicePersona
        self.sessionVoiceSpeed = self.voiceSpeed
        let language = self.activeSttLanguage
        // Nemotron is multilingual — push the BCP-47 language hint before
        // audio starts flowing.
        if language != "en" {
            Task { [weak self] in await self?.nemotronStt.setLanguage(language) }
        }

        self.state = .connecting
        // Advertise `bobe.voice.v1` so a future v2 daemon can route on the
        // subprotocol. Daemon also accepts unversioned upgrades, so this is
        // forward-only — old daemons just ignore the offered protocol.
        var request = URLRequest(url: wsURL)
        DaemonConfig.endpoint.authorize(&request)
        request.setValue(VoiceWire.subprotocolV1, forHTTPHeaderField: "Sec-WebSocket-Protocol")
        let newTask = self.urlSession.webSocketTask(with: request)
        self.task = newTask
        newTask.resume()
        self.receiveLoop()
        self.startKeepalive()

        self.sessionId = "voice-\(Int(Date.now.timeIntervalSince1970))"
        let sid = self.sessionId
        // Per-WS voice prefs sourced from the daemon settings mirror that
        // `refreshDaemonState` keeps current. Sending them explicitly in
        // Hello means the daemon doesn't have to fall back to its config
        // default — the user's pick flows through to this exact session.
        let voiceId = self.sessionVoicePersona
        let speed = self.sessionVoiceSpeed
        let ttsBackend = self.sessionTtsBackend ?? .serverKokoro
        Task { [weak self] in
            guard let self else { return }
            // Validate the selected local TTS before advertising it. Kokoro is
            // validated by the daemon during Hello.
            if ttsBackend == .clientSupertonic {
                do {
                    try await self.clientTtsEngine.prepare()
                } catch {
                    let reason = "Supertonic prepare: \(error.localizedDescription)"
                    self.lastError = reason
                    self.tearDownConnection(finalState: .failed(reason), sendAbort: false)
                    return
                }
            }
            // After a previous disconnect(), tearDownAudio() removed the input
            // tap and set isWarm=false. prewarm() is the only place the engine
            // is configured, and it's idempotent — call it here so the second
            // (and Nth) connect() rebuilds the audio path. Without this, a
            // close→reopen leaves the WS connected but no mic frames flowing.
            guard await self.prewarm() else {
                let reason = self.lastError ?? "voice audio setup failed"
                self.tearDownConnection(finalState: .failed(reason), sendAbort: false)
                return
            }
            await self.sendClient(.hello(
                sessionId: sid,
                playbackRate: UInt32(VoiceWire.ttsOutputSampleRate),
                voiceId: voiceId,
                speed: speed,
                language: language,
                ttsBackend: ttsBackend.rawValue
            ))
        }
    }

    public func disconnect() {
        self.tearDownConnection(finalState: .idle, sendAbort: true)
    }

    /// Canonical terminal teardown. The caller chooses the externally visible
    /// final state so transport failures remain failed rather than being
    /// overwritten by ordinary disconnect cleanup.
    func tearDownConnection(finalState: State, sendAbort: Bool) {
        self.stopKeepalive()
        let oldTask = self.task
        self.task = nil
        self.state = finalState
        self.inputLevel = 0
        self.partialTranscript = ""
        self.pendingPartialForServer = nil
        self.partialSendTask?.cancel()
        self.partialSendTask = nil
        Task {
            if let oldTask {
                if sendAbort,
                   let data = try? Self.jsonEncoder.encode(
                       ClientVoiceMessage.control(action: .abort)
                   ),
                   let text = String(data: data, encoding: .utf8) {
                    try? await oldTask.send(.string(text))
                }
                oldTask.cancel(with: .normalClosure, reason: nil)
            }
        }
        self.tearDownAudio()
        self.sessionSttLanguage = nil
        self.sessionTtsBackend = nil
        self.sessionVoicePersona = nil
        self.sessionVoiceSpeed = nil
    }

    // `tearDownAudio()` lives in `VoicePipeline+Audio.swift` alongside
    // `configureEngine()` — both touch the same internal state and the
    // pairing is clearer when they sit next to each other.

    public func toggle(daemonBaseURL: URL) async {
        switch self.state {
        case .idle, .failed:
            self.connect(daemonBaseURL: daemonBaseURL)
            var waited = 0.0
            while self.state == .connecting, waited < 2.0 {
                try? await Task.sleep(for: .milliseconds(100))
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
        self.cancelClientTtsTurn()
        guard let data = try? Self.jsonEncoder.encode(abort),
              let text = String(data: data, encoding: .utf8)
        else { return }
        Task { [weak self, weak task] in
            try? await task?.send(.string(text))
            await MainActor.run {
                self?.state = .cancelling
            }
        }
    }

    // Audio engine setup + per-frame input handling lives in
    // `VoicePipeline+Audio.swift`. The WS receive loop, control-message
    // dispatch, applyPhase, keepalive helpers, and the canonical
    // `sendClient(_:)` write path live in `VoicePipeline+WebSocket.swift`.
    // CoreAudio default-input/output listeners live in
    // `VoicePipeline+AudioDevice.swift`. Splits are purely organizational
    // — the access surface is identical via the extensions.
}

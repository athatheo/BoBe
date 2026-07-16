import Foundation
import OSLog

private let logger = Logger(subsystem: "com.bobe.app", category: "VoicePipeline.WebSocket")

/// WebSocket-side of the voice pipeline — receive loop, daemon control-
/// message dispatch, phase application, keepalive, and the canonical
/// `sendClient(_:)` write path.
///
/// Split out of `VoicePipeline.swift` (the type definition + lifecycle)
/// purely to keep the core file under the SwiftLint file-length cap.
/// All members are `internal` rather than `private` so they can live in
/// this separate file — the convention is that nothing outside the
/// `Voice/` directory calls these directly.
@MainActor
extension VoicePipeline {
    // MARK: - Receive loop

    func receiveLoop() {
        guard let currentTask = self.task else { return }
        currentTask.receive { [weak self, weak currentTask] result in
            Task { @MainActor [weak self] in
                guard let self else { return }
                // Stale callback from an old WS task — discard.
                guard self.task === currentTask else { return }
                switch result {
                case let .success(msg):
                    await self.handleIncoming(msg)
                    self.receiveLoop()
                case let .failure(err):
                    let reason = err.localizedDescription
                    self.lastError = "recv: \(reason)"
                    logger.error("recv: \(reason, privacy: .public)")
                    self.tearDownConnection(finalState: .failed(reason), sendAbort: false)
                }
            }
        }
    }

    private func handleIncoming(_ msg: URLSessionWebSocketTask.Message) async {
        switch msg {
        case let .string(text):
            await self.handleControlJson(text)
        case let .data(bytes):
            self.handleAudioFrame(bytes)
        @unknown default:
            break
        }
    }

    private func handleControlJson(_ text: String) async {
        guard let data = text.data(using: .utf8) else { return }
        let decoded: ServerVoiceMessage
        do {
            decoded = try Self.jsonDecoder.decode(ServerVoiceMessage.self, from: data)
        } catch {
            logger.warning("malformed control JSON: \(text, privacy: .public)")
            return
        }
        switch decoded {
        case let .helloAck(voicePack, _):
            logger.info("voice.hello_ack voicePack=\(voicePack, privacy: .public)")
        case let .state(phase, _):
            self.applyPhase(phase)
        case let .transcriptFinal(_, transcript):
            if !transcript.isEmpty {
                BobeStore.shared.appendUserVoiceMessage(transcript)
            }
        case let .ttsEnd(turnId):
            self.partialTranscript = ""
            await self.finishClientTtsTurn(turnId: turnId)
        case let .ttsText(turnId, sequence, text):
            await self.enqueueClientTts(turnId: turnId, sequence: sequence, text: text)
        case let .truncate(_, keepMs):
            if self.clientTtsActive {
                self.cancelClientTtsTurn()
            } else {
                self.truncatePlayback(keepMs: keepMs)
            }
        case let .error(code, message):
            self.lastError = "\(code): \(message)"
            logger.error("server: \(code, privacy: .public): \(message, privacy: .public)")
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
        case .idle:
            if self.state != .idle {
                self.state = .idle
            }
            self.stopTtsLevelDecay()
        case .listening:
            if self.clientTtsActive {
                self.pendingServerListening = true
                return
            }
            if self.state != .listening {
                self.state = .listening
            }
            self.stopTtsLevelDecay()
            self.drainPendingAudioDeviceRebuild()
        case .thinking:
            if self.state != .thinking {
                self.state = .thinking
            }
            self.stopTtsLevelDecay()
        case .speaking:
            // New turn — reset the per-turn schedule bookkeeping. Capture
            // the current player sampleTime as the baseline so
            // playedFramesThisTurn returns 0 right at turn start.
            if self.state != .speaking {
                self.scheduledChunks.removeAll(keepingCapacity: true)
                self.nextScheduleFrame = 0
                self.turnFrameBase = 0
                self.sampleTimeBase = self.currentPlayerSampleTime()
                self.startTtsLevelDecay()
                self.state = .speaking
            }
            self.ensureDecoder()
        }
    }

    // MARK: - Keepalive

    func startKeepalive() {
        self.keepaliveTask?.cancel()
        self.keepaliveTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .nanoseconds(self?.keepalivePingIntervalNs ?? 25_000_000_000))
                guard !Task.isCancelled else { return }
                guard let self, let task = self.task else { return }
                task.sendPing { error in
                    if let error {
                        logger.warning(
                            "voice keepalive ping failed: \(error.localizedDescription, privacy: .public)"
                        )
                    }
                }
            }
        }
    }

    func stopKeepalive() {
        self.keepaliveTask?.cancel()
        self.keepaliveTask = nil
    }

    // MARK: - Outbound

    /// Canonical client-message write path. All control JSON sent to the
    /// daemon flows through here — encoders are shared (see `Self.jsonEncoder`)
    /// so we don't allocate per-frame during transcript-partial bursts.
    func sendClient(_ msg: ClientVoiceMessage) async {
        guard let task = self.task else { return }
        guard let data = try? Self.jsonEncoder.encode(msg),
              let text = String(data: data, encoding: .utf8)
        else { return }
        do {
            try await task.send(.string(text))
        } catch {
            let reason = error.localizedDescription
            self.lastError = "ws send text: \(reason)"
            logger.error("ws send text: \(reason, privacy: .public)")
            guard self.task === task else { return }
            self.tearDownConnection(finalState: .failed(reason), sendAbort: false)
        }
    }
}

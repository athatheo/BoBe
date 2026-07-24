@preconcurrency import AVFoundation
import Foundation
import OSLog

private let bodyAdapterLogger = Logger(
    subsystem: "com.bobe.app",
    category: "BodySpeechAdapter"
)

actor BodySpeechAdapter {
    static let shared = BodySpeechAdapter()

    private let session = URLSession(configuration: .default)
    private let stt = FluidAudioStt()
    private var baseURL: URL?
    private var token: String?
    private var socket: URLSessionWebSocketTask?
    private var connectionLoop: Task<Void, Never>?
    private var activeRoute: BodyRoute?
    private var captureOpen = false
    private var eouSegments: [String] = []
    private var decoder: OpusDecoder?
    private var expectedMicrophoneSequence: UInt32?
    private var contractValidated = false

    func start(configuration: BodyAdapterConfiguration) {
        guard self.connectionLoop == nil else { return }
        self.baseURL = configuration.baseURL
        self.token = configuration.bearerToken
        self.connectionLoop = Task { [weak self] in
            await self?.run()
        }
    }

    func stop() async {
        self.connectionLoop?.cancel()
        self.connectionLoop = nil
        self.socket?.cancel(with: .goingAway, reason: nil)
        self.socket = nil
        await self.clearTurn()
        self.baseURL = nil
        self.token = nil
    }

    private func run() async {
        do {
            try await self.prepareStt()
        } catch {
            bodyAdapterLogger.error(
                "Body speech models unavailable: \(error.localizedDescription, privacy: .public)"
            )
            self.connectionLoop = nil
            return
        }

        var attempt = 0
        while !Task.isCancelled {
            do {
                try await self.connectAndReceive()
                attempt = 0
            } catch is CancellationError {
                return
            } catch {
                attempt += 1
                bodyAdapterLogger.warning(
                    "Body adapter disconnected: \(error.localizedDescription, privacy: .public)"
                )
            }
            self.socket?.cancel(with: .goingAway, reason: nil)
            self.socket = nil
            await self.clearTurn()
            let delay = min(pow(2, Double(max(0, attempt - 1))) * 0.5, 15)
            do {
                try await Task.sleep(for: .seconds(delay))
            } catch {
                return
            }
        }
    }

    private func prepareStt() async throws {
        try await self.stt.loadModels(
            onPartial: { [weak self] text in
                await self?.handlePartial(text)
            },
            onEou: { [weak self] text in
                await self?.handleEou(text)
            },
            onProgress: { _ in }
        )
    }

    private func connectAndReceive() async throws {
        guard let baseURL = self.baseURL,
              let token = self.token,
              var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)
        else {
            throw BodySpeechAdapterError.invalidConfiguration
        }
        components.scheme = "ws"
        components.path = "/body/adapter"
        guard let url = components.url else {
            throw BodySpeechAdapterError.invalidConfiguration
        }
        var request = URLRequest(url: url)
        DaemonEndpoint.remote(baseURL: baseURL, bearerToken: token).authorize(&request)
        request.setValue("bobe.speech-adapter.v1", forHTTPHeaderField: "Sec-WebSocket-Protocol")
        let socket = self.session.webSocketTask(with: request)
        self.socket = socket
        self.contractValidated = false
        socket.resume()
        try await self.send(.hello)
        bodyAdapterLogger.info("Body speech adapter connected")

        while !Task.isCancelled {
            switch try await socket.receive() {
            case let .string(text):
                try await self.handleControl(text)
            case let .data(data):
                try await self.handleMedia(data)
            @unknown default:
                throw BodySpeechAdapterError.unsupportedFrame
            }
        }
    }

    private func handleControl(_ text: String) async throws {
        guard let data = text.data(using: .utf8) else {
            throw BodySpeechAdapterError.malformedControl
        }
        switch try JSONDecoder().decode(BodyAdapterServerMessage.self, from: data) {
        case let .welcome(protocolMajor, protocolMinor, adapterGeneration):
            guard protocolMajor == 1, protocolMinor == 1 else {
                throw BodySpeechAdapterError.incompatibleContract
            }
            self.contractValidated = true
            bodyAdapterLogger.info(
                "Body adapter registered generation=\(adapterGeneration, privacy: .public)"
            )
        case let .captureOpen(route, format):
            guard self.contractValidated, format == .capture else {
                throw BodySpeechAdapterError.incompatibleContract
            }
            guard self.activeRoute == nil else {
                throw BodySpeechAdapterError.routeConflict
            }
            self.captureOpen = false
            self.eouSegments.removeAll(keepingCapacity: true)
            self.expectedMicrophoneSequence = nil
            self.decoder = nil
            try await self.stt.reset()
            guard self.socket != nil, self.activeRoute == nil else {
                throw BodySpeechAdapterError.notConnected
            }
            self.activeRoute = route
            self.captureOpen = true
        case let .captureClose(route):
            guard route == self.activeRoute, self.captureOpen else {
                throw BodySpeechAdapterError.staleRoute
            }
            self.captureOpen = false
            try await self.finishCapture(route: route)
        case let .turnCancel(route, _):
            guard route == self.activeRoute else { return }
            await self.clearTurn()
        case let .turnComplete(route, _):
            guard route == self.activeRoute else { return }
            await self.clearTurn()
        case .unknown:
            throw BodySpeechAdapterError.malformedControl
        }
    }

    private func handleMedia(_ data: Data) async throws {
        guard self.contractValidated,
              let (header, payload) = BodyAdapterMediaHeader.parse(data),
              let route = self.activeRoute,
              header.connectionGeneration == route.connectionGeneration,
              header.leaseId == route.leaseId
        else {
            throw BodySpeechAdapterError.staleRoute
        }
        switch header.kind {
        case .microphonePcm:
            guard self.captureOpen,
                  let frame = BodyLinkMicrophoneFrame.parse(payload),
                  frame.streamId == route.captureStreamId,
                  frame.sequence == header.sequence,
                  self.expectedMicrophoneSequence == nil
                    || self.expectedMicrophoneSequence == frame.sequence
                   || frame.hasDiscontinuity
            else {
                throw BodySpeechAdapterError.malformedMedia
            }
            self.expectedMicrophoneSequence = frame.sequence &+ 1
            try await self.stt.acceptSamples(Self.floatSamples(from: frame.pcm))
        case .ttsOpus:
            try await self.decodeAndReturnPcm(header: header, payload: payload)
        case .speakerPcm:
            throw BodySpeechAdapterError.unsupportedFrame
        }
    }

    private func handlePartial(_ text: String) async {
        guard self.captureOpen, let route = self.activeRoute else { return }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            try await self.send(.transcriptPartial(route: route, text: trimmed))
        } catch {
            bodyAdapterLogger.warning(
                "Body partial transcript failed: \(error.localizedDescription, privacy: .public)"
            )
        }
    }

    private func handleEou(_ text: String) {
        guard self.activeRoute != nil else { return }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            self.eouSegments.append(trimmed)
        }
    }

    private func finishCapture(route: BodyRoute) async throws {
        let finished = try await self.stt.finish()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let transcript = combineBodyTranscriptSegments(
            self.eouSegments,
            remainder: finished
        )
        if transcript.isEmpty {
            try await self.send(.transcriptEmpty(route: route))
        } else {
            try await self.send(.transcriptFinal(route: route, text: transcript))
        }
    }

    private func decodeAndReturnPcm(
        header: BodyAdapterMediaHeader,
        payload: Data
    ) async throws {
        guard let parsed = TtsFrameHeader.parse(payload) else {
            throw BodySpeechAdapterError.malformedMedia
        }
        let format = AVAudioFormat(
            commonFormat: .pcmFormatInt16,
            sampleRate: Double(VoiceWire.ttsOutputSampleRate),
            channels: 1,
            interleaved: false
        )
        guard let format,
              let buffer = AVAudioPCMBuffer(
                  pcmFormat: format,
                  // Match the proven Mac playback path: Apple's Opus converter
                  // requires room for the protocol's 60 ms maximum even though
                  // BoBe emits 20 ms packets.
                  frameCapacity: AVAudioFrameCount(VoiceWire.ttsOutputSampleRate * 3 / 50)
              )
        else {
            throw BodySpeechAdapterError.audioBuffer
        }
        if self.decoder == nil {
            self.decoder = try OpusDecoder(outputFormat: format)
        }
        try parsed.payload.withUnsafeBytes { raw in
            try self.decoder?.decode(raw.bindMemory(to: UInt8.self), to: buffer)
        }
        let expectedFrames = AVAudioFrameCount(VoiceWire.ttsOutputSampleRate / 50)
        guard buffer.frameLength > 0, buffer.frameLength <= expectedFrames else {
            bodyAdapterLogger.error(
                "Unexpected decoded frame count: \(buffer.frameLength, privacy: .public)"
            )
            throw BodySpeechAdapterError.audioBuffer
        }
        guard let channel = buffer.int16ChannelData?[0] else {
            bodyAdapterLogger.error("Decoded Opus buffer has no Int16 channel")
            throw BodySpeechAdapterError.audioBuffer
        }
        var pcm = Data(
            bytes: channel,
            count: Int(buffer.frameLength) * MemoryLayout<Int16>.size
        )
        // Apple's decoder applies Opus pre-skip to the first packet (typically
        // 420 rather than 480 samples at 24 kHz). Prefix silence represents
        // that stream-start priming without inserting a gap after speech.
        if pcm.count < 960 {
            var padded = Data(repeating: 0, count: 960 - pcm.count)
            padded.append(pcm)
            pcm = padded
        }
        let response = BodyAdapterMediaHeader(
            kind: .speakerPcm,
            connectionGeneration: header.connectionGeneration,
            leaseId: header.leaseId,
            streamId: header.streamId,
            sequence: header.sequence
        ).encode(payload: pcm)
        guard let socket = self.socket else {
            throw BodySpeechAdapterError.notConnected
        }
        try await socket.send(.data(response))
    }

    private func clearTurn() async {
        self.activeRoute = nil
        self.captureOpen = false
        self.eouSegments.removeAll(keepingCapacity: true)
        self.decoder = nil
        self.expectedMicrophoneSequence = nil
        try? await self.stt.reset()
    }

    private func send(_ message: BodyAdapterClientMessage) async throws {
        guard let socket = self.socket else {
            throw BodySpeechAdapterError.notConnected
        }
        let data = try JSONEncoder().encode(message)
        guard let text = String(data: data, encoding: .utf8) else {
            throw BodySpeechAdapterError.malformedControl
        }
        try await socket.send(.string(text))
    }

    private static func floatSamples(from pcm: Data) -> [Float] {
        var samples = [Float]()
        samples.reserveCapacity(pcm.count / 2)
        var index = pcm.startIndex
        while index < pcm.endIndex {
            let low = UInt16(pcm[index])
            let highIndex = pcm.index(after: index)
            let high = UInt16(pcm[highIndex])
            samples.append(Float(Int16(bitPattern: low | (high << 8))) / 32_768)
            index = pcm.index(highIndex, offsetBy: 1)
        }
        return samples
    }
}

func combineBodyTranscriptSegments(
    _ segments: [String],
    remainder: String
) -> String {
    var parts = segments
        .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        .filter { !$0.isEmpty }
    let remainder = remainder.trimmingCharacters(in: .whitespacesAndNewlines)
    if !remainder.isEmpty, parts.last != remainder {
        parts.append(remainder)
    }
    return parts.joined(separator: " ")
}

private enum BodySpeechAdapterError: LocalizedError {
    case invalidConfiguration
    case unsupportedFrame
    case malformedControl
    case malformedMedia
    case routeConflict
    case staleRoute
    case audioBuffer
    case notConnected
    case incompatibleContract

    var errorDescription: String? {
        switch self {
        case .invalidConfiguration: "Invalid body adapter configuration"
        case .unsupportedFrame: "Unsupported body adapter frame"
        case .malformedControl: "Malformed body adapter control"
        case .malformedMedia: "Malformed body adapter media"
        case .routeConflict: "A different body route is already active"
        case .staleRoute: "Body adapter route is stale"
        case .audioBuffer: "Unable to decode body audio"
        case .notConnected: "Body speech adapter is not connected"
        case .incompatibleContract: "Body speech adapter protocol or audio format is incompatible"
        }
    }
}

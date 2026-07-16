@preconcurrency import AVFoundation
import Foundation
import os

/// Thrown when `VoicePipeline.ensureSttLoaded` exceeds its deadline.
struct VoiceLoadTimeout: Error {}

/// Runtime errors raised from inside `VoicePipeline.configureEngine`.
enum VoiceError: Error, CustomStringConvertible {
    case runtime(String)
    var description: String {
        switch self {
        case let .runtime(s): s
        }
    }
}

/// Race an async operation against a deadline. Throws `VoiceLoadTimeout`
/// if the operation doesn't return before the deadline elapses.
func withVoiceLoadTimeout(
    seconds: TimeInterval,
    _ operation: @escaping @Sendable () async throws -> Void
) async throws {
    try await withThrowingTaskGroup(of: Void.self) { group in
        group.addTask { try await operation() }
        group.addTask {
            try await Task.sleep(for: .seconds(seconds))
            throw VoiceLoadTimeout()
        }
        // First task to finish wins; cancel the other.
        _ = try await group.next()
        group.cancelAll()
    }
}

/// `http(s)://host:port/prefix` → `ws(s)://host:port/prefix/voice/stream`.
/// Returns nil if the base URL is malformed.
func voiceWsEndpoint(from baseURL: URL) -> URL? {
    guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
        return nil
    }
    let scheme = components.scheme?.lowercased()
    components.scheme = (scheme == "https") ? "wss" : "ws"
    let prefix = components.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    components.path = prefix.isEmpty ? "/voice/stream" : "/\(prefix)/voice/stream"
    return components.url
}

/// One-line dump of an `AVAudioFormat` for log lines, e.g. "16000Hz 1ch f32".
func describe(_ format: AVAudioFormat) -> String {
    let fmtName: String = switch format.commonFormat {
    case .pcmFormatFloat32: "f32"
    case .pcmFormatFloat64: "f64"
    case .pcmFormatInt16: "i16"
    case .pcmFormatInt32: "i32"
    default: "?"
    }
    return "\(Int(format.sampleRate))Hz \(format.channelCount)ch \(fmtName)"
        + (format.isInterleaved ? "" : " (planar)")
}

/// Run `AVAudioConverter.convert(to:error:withInputFrom:)` with the
/// "feed this buffer exactly once" pattern used by every call site
/// (Opus decode, Parakeet input resample, Nemotron input resample). The
/// converter's input block runs in a loop until `.noDataNow` is
/// returned, so the once-flag lives in a reference type so the
/// `@Sendable` closure can mutate it.
///
func convertSingleBuffer(
    _ converter: AVAudioConverter,
    source: AVAudioBuffer,
    into output: AVAudioPCMBuffer
) -> (status: AVAudioConverterOutputStatus, error: NSError?) {
    let state = OSAllocatedUnfairLock(initialState: false)
    var err: NSError?
    let status = converter.convert(to: output, error: &err) { _, statusPtr in
        let shouldFeed = state.withLock { fed in
            guard !fed else { return false }
            fed = true
            return true
        }
        if !shouldFeed {
            statusPtr.pointee = .noDataNow
            return nil
        }
        statusPtr.pointee = .haveData
        return source
    }
    return (status, err)
}

/// Realtime-safe RMS computation for the TTS playerNode tap. Supports the
/// two formats AVAudioMixerNode is likely to surface here: Float32 (the
/// audio engine's canonical mixer format) and Int16 (the format we ask
/// for at connect time). No allocations, no main-actor hop — meant to run
/// straight from the realtime audio dispatch queue.
func computePlaybackRms(_ buffer: AVAudioPCMBuffer) -> Float {
    let frames = Int(buffer.frameLength)
    guard frames > 0 else { return 0 }
    var sumSq: Double = 0
    if let f32 = buffer.floatChannelData {
        let samples = f32[0]
        for i in 0 ..< frames {
            let s = Double(samples[i])
            sumSq += s * s
        }
    } else if let i16 = buffer.int16ChannelData {
        let samples = i16[0]
        for i in 0 ..< frames {
            let s = Double(samples[i]) / 32_768.0
            sumSq += s * s
        }
    } else {
        return 0
    }
    return Float(sqrt(sumSq / Double(frames)))
}

@preconcurrency import AVFoundation
import Foundation

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

/// `http(s)://host:port` → `ws(s)://host:port/voice/stream`. Returns nil if
/// the base URL is malformed.
func voiceWsEndpoint(from baseURL: URL) -> URL? {
    guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
        return nil
    }
    let scheme = components.scheme?.lowercased()
    components.scheme = (scheme == "https") ? "wss" : "ws"
    components.path = "/voice/stream"
    return components.url
}

/// Deep-copy the tap buffer's float channel data into a fresh
/// `AVAudioPCMBuffer` that's safe to send across `Task { ... }` hops.
/// Apple's docs state tap buffer storage may be reused after the
/// `installTap` block returns; capturing the raw buffer across an async
/// hop lets the realtime queue clobber bytes before the consumer reads
/// them. Returns nil on alloc failure or if the source isn't Float32.
func copyPcmFloatBuffer(_ src: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
    guard let copy = AVAudioPCMBuffer(pcmFormat: src.format, frameCapacity: src.frameCapacity)
    else {
        return nil
    }
    copy.frameLength = src.frameLength
    guard let srcChannels = src.floatChannelData, let dstChannels = copy.floatChannelData else {
        return nil
    }
    let frameCount = Int(src.frameLength)
    let bytes = frameCount * MemoryLayout<Float>.size
    for ch in 0 ..< Int(src.format.channelCount) {
        memcpy(dstChannels[ch], srcChannels[ch], bytes)
    }
    return copy
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
/// (Opus decode, Parakeet input resample, Qwen3 input resample). The
/// converter's input block runs in a loop until `.noDataNow` is
/// returned, so the once-flag lives in a reference type so the
/// `@Sendable` closure can mutate it.
///
/// Safety: `FedState` is `@unchecked Sendable` by convention, not by
/// synchronization. `AVAudioConverter.convert(to:error:withInputFrom:)`
/// invokes its input closure synchronously on the calling thread for
/// the duration of one `convert` call — there is no cross-thread
/// access, so the `var fed` read/write happens on a single thread.
/// The `@unchecked` is documentation of this convention so the
/// `@Sendable` input-closure signature compiles.
func convertSingleBuffer(
    _ converter: AVAudioConverter,
    source: AVAudioBuffer,
    into output: AVAudioPCMBuffer
) -> (status: AVAudioConverterOutputStatus, error: NSError?) {
    final class FedState: @unchecked Sendable { var fed = false }
    let state = FedState()
    var err: NSError?
    let status = converter.convert(to: output, error: &err) { _, statusPtr in
        if state.fed {
            statusPtr.pointee = .noDataNow
            return nil
        }
        state.fed = true
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

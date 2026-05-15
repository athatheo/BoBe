import AVFoundation
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
            try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
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

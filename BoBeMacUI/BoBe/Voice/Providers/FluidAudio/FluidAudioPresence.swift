import Foundation

/// Static-only surface shared by every FluidAudio model presence type
/// (Parakeet today, Qwen3 today, future variants). Lets settings UI map
/// `VoicePipeline.SttStatus → VoiceModelCard.Status` once instead of per
/// engine. The 4 statics here are the entire contract a card needs.
protocol FluidAudioPresence {
    static func isInstalled() -> Bool
    static var approximateTotalBytes: UInt64 { get }
    static func observedBytes() -> UInt64
    static func observedPercent() -> Int
}

extension FluidAudioPresence {
    /// Default percent: floor before install completes (cap at 99 so the UI
    /// never lies about completion), 100 once the sentinels land.
    static func observedPercent() -> Int {
        if isInstalled() { return 100 }
        let pct = Int(
            Double(Self.observedBytes()) / Double(Self.approximateTotalBytes) * 100.0
        )
        return max(0, min(99, pct))
    }
}

extension FluidAudioModelPresence: FluidAudioPresence {}
extension FluidAudioQwen3ModelPresence: FluidAudioPresence {}

/// Shared filesystem helpers for the FluidAudio cache. Both presence enums
/// previously inlined byte-identical copies of these.
enum FluidAudioCache {
    /// Default FluidAudio cache root (matches the SDK's `ModelRegistry`
    /// default). We don't override `ModelRegistry.baseURL` so other apps
    /// using FluidAudio can share this cache.
    static var root: URL {
        let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first ?? FileManager.default.homeDirectoryForCurrentUser
        return appSupport
            .appendingPathComponent("FluidAudio", isDirectory: true)
            .appendingPathComponent("Models", isDirectory: true)
    }

    /// Total bytes resident under `dir`. Skips hidden files. Returns 0 if
    /// `dir` doesn't exist yet — used for download-progress polling so a
    /// missing dir reads as "0 bytes downloaded" rather than throwing.
    static func directorySizeBytes(at dir: URL) -> UInt64 {
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(
            at: dir,
            includingPropertiesForKeys: [.isRegularFileKey, .totalFileAllocatedSizeKey],
            options: [.skipsHiddenFiles]
        )
        else {
            return 0
        }
        var total: UInt64 = 0
        for case let url as URL in enumerator {
            let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .totalFileAllocatedSizeKey])
            if values?.isRegularFile == true, let size = values?.totalFileAllocatedSize {
                total += UInt64(size)
            }
        }
        return total
    }
}

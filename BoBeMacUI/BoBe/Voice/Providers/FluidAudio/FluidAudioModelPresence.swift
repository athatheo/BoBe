import Foundation

/// Cheap filesystem probe for the Parakeet EOU 320ms model. Avoids the
/// ~2s `loadModels()` ANE warm-up on boot decision; the real load happens
/// lazily in the background after a positive check. Three sentinel
/// `.mlmodelc` packages — checking all 23 files is overkill.
enum FluidAudioModelPresence {
    /// Default FluidAudio cache root (matches the SDK's `ModelRegistry`
    /// default). We don't override `ModelRegistry.baseURL` so other apps
    /// using FluidAudio can share this cache.
    static var cacheRoot: URL {
        let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first ?? FileManager.default.homeDirectoryForCurrentUser
        return appSupport
            .appendingPathComponent("FluidAudio", isDirectory: true)
            .appendingPathComponent("Models", isDirectory: true)
    }

    /// Path the Parakeet EOU 320ms variant unpacks to.
    static var parakeetEou320msDirectory: URL {
        cacheRoot
            .appendingPathComponent("parakeet-eou-streaming", isDirectory: true)
            .appendingPathComponent("parakeet-eou-streaming", isDirectory: true)
            .appendingPathComponent("320ms", isDirectory: true)
    }

    /// Sentinel files inside the variant directory whose presence indicates
    /// the install is complete. These are the compiled CoreML model
    /// packages — `loadModels` fails immediately without them, so a strong
    /// signal. Verified against the on-disk layout (parakeet-eou-streaming/
    /// parakeet-eou-streaming/320ms/) which ships these three plus their
    /// `.mlpackage` source twins + metadata JSON.
    static let sentinelFiles: [String] = [
        "streaming_encoder.mlmodelc",
        "decoder.mlmodelc",
        "joint_decision.mlmodelc",
    ]

    /// True if every sentinel exists at the expected path. Cheap — three
    /// `fileExists` syscalls.
    static func isInstalled() -> Bool {
        let dir = parakeetEou320msDirectory
        let fm = FileManager.default
        for sentinel in sentinelFiles {
            let path = dir.appendingPathComponent(sentinel, isDirectory: true).path
            if !fm.fileExists(atPath: path) {
                return false
            }
        }
        return true
    }

    /// Approximate full install size. FluidAudio doesn't expose this as
    /// metadata so the value is observed-empirically — see what an actual
    /// completed install weighs in the SDK's tests. Used as the denominator
    /// for download progress until the SDK adds a progress callback.
    static let approximateTotalBytes: UInt64 = 600_000_000

    /// Best-effort progress bytes observed via filesystem polling. Counts
    /// every file under the variant's parent dir so partial downloads, the
    /// `.mlpackage`/`.mlmodelc` pairs, and metadata JSONs all contribute.
    /// Returns 0 when the dir doesn't exist yet.
    static func observedBytes() -> UInt64 {
        let root = cacheRoot.appendingPathComponent("parakeet-eou-streaming", isDirectory: true)
        return Self.directorySizeBytes(at: root)
    }

    /// 0...100 — rough percent based on `observedBytes / approximateTotalBytes`.
    /// Caps at 99 until `isInstalled()` is true (so the UI never claims 100%
    /// before the sentinel files actually land).
    static func observedPercent() -> Int {
        if isInstalled() { return 100 }
        let pct = Int(Double(observedBytes()) / Double(approximateTotalBytes) * 100.0)
        return max(0, min(99, pct))
    }

    private static func directorySizeBytes(at root: URL) -> UInt64 {
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(
            at: root,
            includingPropertiesForKeys: [.isRegularFileKey, .totalFileAllocatedSizeKey],
            options: [.skipsHiddenFiles]
        ) else {
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

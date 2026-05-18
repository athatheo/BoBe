import Foundation

/// Cheap filesystem probe for the Parakeet EOU 320ms model. Avoids the
/// ~2s `loadModels()` ANE warm-up on boot decision; the real load happens
/// lazily in the background after a positive check. Three sentinel
/// `.mlmodelc` packages — checking all 23 files is overkill.
enum FluidAudioModelPresence {
    /// Path the Parakeet EOU 320ms variant unpacks to.
    static var parakeetEou320msDirectory: URL {
        FluidAudioCache.root
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
        let dir = self.parakeetEou320msDirectory
        let fm = FileManager.default
        for sentinel in self.sentinelFiles {
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
        let root = FluidAudioCache.root.appendingPathComponent(
            "parakeet-eou-streaming",
            isDirectory: true
        )
        return FluidAudioCache.directorySizeBytes(at: root)
    }
}

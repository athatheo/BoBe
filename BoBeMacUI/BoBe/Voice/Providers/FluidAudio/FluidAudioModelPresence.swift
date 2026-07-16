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
}

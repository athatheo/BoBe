import Foundation

/// Filesystem-level presence check for the FluidAudio Qwen3-ASR (Mandarin)
/// model bundle plus its companion Silero VAD model. Mirrors
/// `FluidAudioModelPresence` but for the Mandarin engine combo.
///
/// Qwen3 ships as a 2-model CoreML pipeline (`audio_encoder_v2.mlmodelc` +
/// `decoder_stateful.mlmodelc`) plus a `qwen3_asr_embeddings.bin` weight
/// matrix and `vocab.json`. The VAD is a single `silero-vad-unified-256ms-
/// v6.0.0.mlmodelc` that lives in a sibling directory because FluidAudio's
/// `VadManager` caches it under its own repo folder.
enum FluidAudioQwen3ModelPresence {
    /// Default FluidAudio cache root (matches the SDK's `ModelRegistry`).
    /// Identical to `FluidAudioModelPresence.cacheRoot` — we don't override
    /// `ModelRegistry.baseURL` so cross-app sharing keeps working.
    static var cacheRoot: URL {
        let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first ?? FileManager.default.homeDirectoryForCurrentUser
        return appSupport
            .appendingPathComponent("FluidAudio", isDirectory: true)
            .appendingPathComponent("Models", isDirectory: true)
    }

    /// Qwen3 cache layout: `qwen3-asr-0.6b-coreml/f32/`. We pin to the f32
    /// (full-precision) variant — int8 is half the size but worse quality
    /// and the user expects English-Parakeet parity for accuracy.
    static var qwen3F32Directory: URL {
        cacheRoot
            .appendingPathComponent("qwen3-asr-0.6b-coreml", isDirectory: true)
            .appendingPathComponent("f32", isDirectory: true)
    }

    /// VAD cache layout: `silero-vad-coreml/silero-vad-unified-256ms-v6.0.0.mlmodelc`.
    /// Lives in a sibling directory because FluidAudio's `VadManager` caches
    /// independently from ASR models.
    static var sileroVadDirectory: URL {
        cacheRoot
            .appendingPathComponent("silero-vad-coreml", isDirectory: true)
    }

    /// Required Qwen3 files for the 2-model "Full" pipeline (Swift-side
    /// embedding lookup, fused lmHead). Source: `ModelNames.Qwen3ASR.
    /// requiredModelsFull` in FluidAudio.
    static let qwen3SentinelFiles: [String] = [
        "qwen3_asr_audio_encoder_v2.mlmodelc",
        "qwen3_asr_decoder_stateful.mlmodelc",
        "qwen3_asr_embeddings.bin",
        "vocab.json",
    ]

    /// VAD sentinel — a single compiled CoreML package. Source:
    /// `ModelNames.VAD.sileroVadFile`.
    static let vadSentinelFile = "silero-vad-unified-256ms-v6.0.0.mlmodelc"

    /// True if both Qwen3 + Silero VAD are on disk.
    static func isInstalled() -> Bool {
        let fm = FileManager.default
        for sentinel in qwen3SentinelFiles {
            let path = qwen3F32Directory.appendingPathComponent(sentinel).path
            if !fm.fileExists(atPath: path) { return false }
        }
        let vadPath = sileroVadDirectory.appendingPathComponent(vadSentinelFile).path
        return fm.fileExists(atPath: vadPath)
    }

    /// Approximate full install size: Qwen3 f32 ~1.75 GB + Silero VAD ~3 MB.
    /// Used as the denominator for download progress percent; FluidAudio
    /// SDK doesn't expose a progress callback today.
    static let approximateTotalBytes: UInt64 = 1_750_000_000

    /// Best-effort progress bytes observed via filesystem polling — counts
    /// every file under both cache subtrees. Returns 0 when neither exists.
    static func observedBytes() -> UInt64 {
        let qwen3Root = cacheRoot
            .appendingPathComponent("qwen3-asr-0.6b-coreml", isDirectory: true)
        return Self.directorySizeBytes(at: qwen3Root)
            + Self.directorySizeBytes(at: sileroVadDirectory)
    }

    /// 0...100 — rough percent based on `observedBytes / approximateTotalBytes`.
    /// Caps at 99 until `isInstalled()` is true so the UI never claims 100%
    /// before the sentinel files actually land.
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

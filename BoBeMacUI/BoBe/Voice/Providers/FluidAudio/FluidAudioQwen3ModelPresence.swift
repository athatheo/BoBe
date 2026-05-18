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
    /// Qwen3 cache layout: `qwen3-asr-0.6b-coreml/f32/`. We pin to the f32
    /// (full-precision) variant — int8 is half the size but worse quality
    /// and the user expects English-Parakeet parity for accuracy.
    static var qwen3F32Directory: URL {
        FluidAudioCache.root
            .appendingPathComponent("qwen3-asr-0.6b-coreml", isDirectory: true)
            .appendingPathComponent("f32", isDirectory: true)
    }

    /// VAD cache layout: `silero-vad-coreml/silero-vad-unified-256ms-v6.0.0.mlmodelc`.
    /// Lives in a sibling directory because FluidAudio's `VadManager` caches
    /// independently from ASR models.
    static var sileroVadDirectory: URL {
        FluidAudioCache.root
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
        for sentinel in self.qwen3SentinelFiles {
            let path = self.qwen3F32Directory.appendingPathComponent(sentinel).path
            if !fm.fileExists(atPath: path) { return false }
        }
        let vadPath = self.sileroVadDirectory.appendingPathComponent(self.vadSentinelFile).path
        return fm.fileExists(atPath: vadPath)
    }

    /// Approximate full install size: Qwen3 f32 ~1.75 GB + Silero VAD ~3 MB.
    /// Used as the denominator for download progress percent; FluidAudio
    /// SDK doesn't expose a progress callback today.
    static let approximateTotalBytes: UInt64 = 1_750_000_000

    /// Best-effort progress bytes observed via filesystem polling — counts
    /// every file under both cache subtrees. Returns 0 when neither exists.
    static func observedBytes() -> UInt64 {
        let qwen3Root = FluidAudioCache.root
            .appendingPathComponent("qwen3-asr-0.6b-coreml", isDirectory: true)
        return FluidAudioCache.directorySizeBytes(at: qwen3Root)
            + FluidAudioCache.directorySizeBytes(at: self.sileroVadDirectory)
    }
}

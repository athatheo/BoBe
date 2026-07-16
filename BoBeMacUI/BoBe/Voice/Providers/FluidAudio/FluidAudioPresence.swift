import FluidAudio
import Foundation

/// Static-only surface shared by every FluidAudio model presence type
/// (Parakeet + Nemotron). Presence is deliberately separate from live
/// progress, which now comes from FluidAudio's ModelHub callback.
protocol FluidAudioPresence {
    static func isInstalled() -> Bool
}

extension FluidAudioModelPresence: FluidAudioPresence {}
extension FluidAudioNemotronModelPresence: FluidAudioPresence {}

func voiceSttProgress(_ progress: DownloadProgress) -> VoiceSttLoadProgress {
    let phase: VoiceSttLoadProgress.Phase = switch progress.phase {
    case .listing:
        .listing
    case let .downloading(completedFiles, totalFiles):
        .downloading(completedFiles: completedFiles, totalFiles: totalFiles)
    case let .compiling(modelName):
        .compiling(modelName: modelName)
    }
    return VoiceSttLoadProgress(
        fractionCompleted: progress.fractionCompleted,
        phase: phase
    )
}

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
}

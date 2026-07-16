import Foundation

enum FluidAudioSupertonicModelPresence {
    static var modelDirectory: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/fluidaudio/Models/supertonic-3", isDirectory: true)
    }

    static func isInstalled() -> Bool {
        let required = [
            "TextEncoder.mlmodelc",
            "DurationPredictor.mlmodelc",
            "Vocoder.mlmodelc",
            "VectorEstimatorVariants/VectorEstimator_L128_int4.mlmodelc",
            "tts.json",
            "unicode_indexer.json",
            "voice_styles/F1.json",
        ]
        return required.allSatisfy {
            FileManager.default.fileExists(
                atPath: self.modelDirectory.appendingPathComponent($0).path
            )
        }
    }
}

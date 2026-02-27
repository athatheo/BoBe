import Foundation

// MARK: - Step Flow

/// Wizard steps — feature-based with skip support
enum SetupStep {
    case chooseMode
    case downloadingEngine
    case downloadingModel
    case captureSetup
    case complete
    case error
}

// MARK: - Model Tiers

/// Model tiers: Small/Medium use unified VL model, Large uses separate LLM + vision
enum ModelSize: String, CaseIterable {
    case small = "qwen3-vl:2b"
    case medium = "qwen3-vl:4b"
    case large = "qwen3:14b"

    var displayName: String {
        switch self {
        case .small: "Small (2B)"
        case .medium: "Medium (4B)"
        case .large: "Large (14B)"
        }
    }

    var sizeDescription: String {
        switch self {
        case .small: "~2.7 GB total"
        case .medium: "~4.1 GB total"
        case .large: "~15 GB total"
        }
    }

    var description: String {
        switch self {
        case .small: "Fast, works on any Mac. Good for getting started."
        case .medium: "Smarter responses. Recommended for 32GB+ RAM."
        case .large: "Best quality. Recommended for 64GB+ RAM."
        }
    }

    /// For Small/Medium the LLM IS the vision model (unified VL)
    var isUnifiedVL: Bool { self != .large }

    /// Separate vision model only needed for Large tier
    var separateVisionModel: String? {
        self == .large ? "qwen3-vl:8b" : nil
    }

    /// Vision model name (same as LLM for small/medium, separate for large)
    var visionModelName: String {
        separateVisionModel ?? rawValue
    }

    var diskRequirement: Int64 {
        switch self {
        case .small: 2_700_000_000
        case .medium: 4_100_000_000
        case .large: 15_000_000_000
        }
    }
}

/// Cloud provider options
enum CloudProvider: String, CaseIterable {
    case openai = "OpenAI"
    case azure = "Azure OpenAI"

    var defaultModel: String {
        switch self {
        case .openai: "gpt-4o-mini"
        case .azure: "gpt-5-mini"
        }
    }
}

enum SetupError: Error, LocalizedError {
    case diskSpace(String)
    var errorDescription: String? {
        switch self {
        case .diskSpace(let msg): msg
        }
    }
}

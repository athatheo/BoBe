import Foundation
import SwiftUI

// MARK: - Step Flow

/// Wizard steps: welcome → aiChoice → cloud/local config → downloading → capture → complete
enum SetupStep {
    case welcome
    case chooseMode
    case cloudConfig
    case localConfig
    case downloadingEngine
    case downloadingModel
    case initializing
    case captureSetup
    case complete
    case error
}

// MARK: - Local Model Options

/// All tiers use separate text (qwen3) and vision (qwen3-vl) models.
struct ModelOption: Identifiable, Equatable {
    let id: String
    let label: String
    let size: String
    let description: String
    let modelName: String
    let visionModel: String

    var diskRequirement: Int64 {
        switch id {
        case "small": 6_000_000_000
        case "medium": 11_000_000_000
        case "large": 15_000_000_000
        default: 6_000_000_000
        }
    }
}

let localModelOptions: [ModelOption] = [
    ModelOption(
        id: "small", label: "Small (4B)", size: "~6 GB",
        description: "Good balance of speed and quality. Any Apple Silicon Mac, 16 GB+ RAM.",
        modelName: "qwen3:4b", visionModel: "qwen3-vl:4b"
    ),
    ModelOption(
        id: "medium", label: "Medium (8B)", size: "~11 GB",
        description: "Better quality. M1 Pro / M2 or newer, 24 GB+ RAM.",
        modelName: "qwen3:8b", visionModel: "qwen3-vl:8b"
    ),
    ModelOption(
        id: "large", label: "Large (14B)", size: "~15 GB",
        description: "Best quality. M2 Pro / M3 or newer, 48 GB+ RAM.",
        modelName: "qwen3:14b", visionModel: "qwen3-vl:8b"
    ),
]

let defaultModelOption: ModelOption = localModelOptions[0]

// MARK: - OpenAI Model Options

struct OpenAIModelOption: Identifiable, Equatable {
    let id: String
    let label: String
    let description: String
    let modelName: String
}

let openAIModelOptions: [OpenAIModelOption] = [
    OpenAIModelOption(
        id: "gpt-5-mini", label: "GPT-5 mini",
        description: "Fast, cost-effective",
        modelName: "gpt-5-mini-2025-08-07"
    ),
    OpenAIModelOption(
        id: "gpt-5-1", label: "GPT-5.1",
        description: "Higher capability",
        modelName: "gpt-5.1-2025-08-07"
    ),
]

let defaultOpenAIModelOption: OpenAIModelOption = openAIModelOptions[0]

// MARK: - Cloud Provider Options

struct OnlineProvider: Identifiable, Equatable {
    let id: String
    let label: String
    let placeholder: String
    let defaultModel: String
    var needsEndpoint: Bool = false
    var endpointPlaceholder: String = ""
}

let onlineProviders: [OnlineProvider] = [
    OnlineProvider(
        id: "openai", label: "OpenAI",
        placeholder: "sk-...", defaultModel: "gpt-5-mini-2025-08-07"
    ),
    OnlineProvider(
        id: "azure_openai", label: "Azure OpenAI",
        placeholder: "Your Azure API key", defaultModel: "gpt-5-mini",
        needsEndpoint: true,
        endpointPlaceholder: "https://your-resource.cognitiveservices.azure.com/openai/v1/"
    ),
]

// MARK: - Setup Mode

enum SetupMode { case local, online }

// MARK: - Errors

enum SetupError: Error, LocalizedError {
    case diskSpace(String)
    case general(String)
    var errorDescription: String? {
        switch self {
        case .diskSpace(let msg): msg
        case .general(let msg): msg
        }
    }
}

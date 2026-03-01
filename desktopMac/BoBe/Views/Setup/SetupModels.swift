import Foundation

// MARK: - Step Flow

enum SetupStep {
    case welcome
    case chooseMode
    case cloudConfig
    case localConfig
    case setupInProgress
    case captureSetup
    case complete
    case error
}

enum SetupMode {
    case local
    case online
}

// MARK: - Backend-Driven Options (from GET /onboarding/options)

struct LocalTier: Codable, Identifiable, Equatable {
    let id: String
    let label: String
    let description: String
    let diskEstimateBytes: Int64

    enum CodingKeys: String, CodingKey {
        case id, label, description
        case diskEstimateBytes = "disk_estimate_bytes"
    }

    var diskLabel: String {
        String(format: "~%.0f GB", Double(self.diskEstimateBytes) / 1_000_000_000)
    }
}

struct CloudProvider: Codable, Identifiable, Equatable {
    let id: String
    let label: String
    let requires: [String]
    let models: [String]
    let recommended: String?

    var needsEndpoint: Bool {
        self.requires.contains("endpoint")
    }

    var needsDeployment: Bool {
        self.requires.contains("deployment")
    }
}

struct OnboardingOptions: Codable {
    let localTiers: [LocalTier]
    let cloudProviders: [CloudProvider]

    enum CodingKeys: String, CodingKey {
        case localTiers = "local_tiers"
        case cloudProviders = "cloud_providers"
    }
}

// MARK: - Setup Job Types (from POST/GET /onboarding/setup)

struct SetupJobStep: Codable, Identifiable {
    let id: String
    let status: String
    let message: String?
    let progress: StepProgressInfo?
}

struct StepProgressInfo: Codable {
    let percent: Int?
    let currentBytes: Int64?
    let totalBytes: Int64?

    enum CodingKeys: String, CodingKey {
        case percent
        case currentBytes = "current_bytes"
        case totalBytes = "total_bytes"
    }
}

struct SetupJobState: Codable {
    let jobId: String
    let status: String
    let currentStep: String?
    let steps: [SetupJobStep]
    let error: String?

    enum CodingKeys: String, CodingKey {
        case jobId = "job_id"
        case status
        case currentStep = "current_step"
        case steps, error
    }

    var isTerminal: Bool {
        ["succeeded", "failed", "canceled"].contains(self.status)
    }

    var overallPercent: Double {
        let total = self.steps.count
        guard total > 0 else { return 0 }
        let completed = self.steps.count(where: { $0.status == "succeeded" || $0.status == "skipped" })
        let currentPct = Double(steps.first { $0.status == "in_progress" }?.progress?.percent ?? 0) / 100.0
        return (Double(completed) + currentPct) / Double(total) * 100
    }
}

struct SetupRequest: Encodable {
    let mode: String
    var tier: String?
    var provider: String?
    var apiKey: String?
    var model: String?
    var endpoint: String?
    var deployment: String?

    enum CodingKeys: String, CodingKey {
        case mode, tier, provider, model, endpoint, deployment
        case apiKey = "api_key"
    }
}

// MARK: - Errors

enum SetupError: Error, LocalizedError {
    case diskSpace(String)
    case general(String)
    var errorDescription: String? {
        switch self {
        case let .diskSpace(msg): msg
        case let .general(msg): msg
        }
    }
}

import Foundation

struct DaemonSettings: Codable, Sendable {
    var llmBackend: String
    var ollamaModel: String
    var openaiModel: String
    var openaiApiKeySet: Bool
    var azureOpenaiEndpoint: String
    var azureOpenaiDeployment: String
    var azureOpenaiApiKeySet: Bool
    var captureEnabled: Bool
    var captureIntervalSeconds: Int
    var checkinEnabled: Bool
    var checkinTimes: [String]
    var checkinJitterMinutes: Int
    var learningEnabled: Bool
    var learningIntervalMinutes: Int
    var conversationInactivityTimeoutSeconds: Int
    var conversationAutoCloseMinutes: Int
    var conversationSummaryEnabled: Bool
    var goalCheckIntervalSeconds: Double
    var projectsDirectory: String
    var toolsEnabled: Bool
    var toolsMaxIterations: Int
    var mcpEnabled: Bool
    var similarityDeduplicationThreshold: Double
    var similaritySearchRecallThreshold: Double
    var similarityClusteringThreshold: Double
    var memoryShortTermRetentionDays: Int
    var memoryLongTermRetentionDays: Int
    var goalWorkerEnabled: Bool
    var goalWorkerAutonomous: Bool
    var goalWorkerMaxConcurrent: Int
    var visionBackend: String
    var visionOllamaModel: String
    var localeOverride: String?
    var effectiveLocale: String
    var supportedLocales: [String]

    enum CodingKeys: String, CodingKey {
        case llmBackend = "llm_backend"
        case ollamaModel = "ollama_model"
        case openaiModel = "openai_model"
        case openaiApiKeySet = "openai_api_key_set"
        case azureOpenaiEndpoint = "azure_openai_endpoint"
        case azureOpenaiDeployment = "azure_openai_deployment"
        case azureOpenaiApiKeySet = "azure_openai_api_key_set"
        case captureEnabled = "capture_enabled"
        case captureIntervalSeconds = "capture_interval_seconds"
        case checkinEnabled = "checkin_enabled"
        case checkinTimes = "checkin_times"
        case checkinJitterMinutes = "checkin_jitter_minutes"
        case learningEnabled = "learning_enabled"
        case learningIntervalMinutes = "learning_interval_minutes"
        case conversationInactivityTimeoutSeconds = "conversation_inactivity_timeout_seconds"
        case conversationAutoCloseMinutes = "conversation_auto_close_minutes"
        case conversationSummaryEnabled = "conversation_summary_enabled"
        case goalCheckIntervalSeconds = "goal_check_interval_seconds"
        case projectsDirectory = "projects_directory"
        case toolsEnabled = "tools_enabled"
        case toolsMaxIterations = "tools_max_iterations"
        case mcpEnabled = "mcp_enabled"
        case similarityDeduplicationThreshold = "similarity_deduplication_threshold"
        case similaritySearchRecallThreshold = "similarity_search_recall_threshold"
        case similarityClusteringThreshold = "similarity_clustering_threshold"
        case memoryShortTermRetentionDays = "memory_short_term_retention_days"
        case memoryLongTermRetentionDays = "memory_long_term_retention_days"
        case goalWorkerEnabled = "goal_worker_enabled"
        case goalWorkerAutonomous = "goal_worker_autonomous"
        case goalWorkerMaxConcurrent = "goal_worker_max_concurrent"
        case visionBackend = "vision_backend"
        case visionOllamaModel = "vision_ollama_model"
        case localeOverride = "locale_override"
        case effectiveLocale = "effective_locale"
        case supportedLocales = "supported_locales"
    }
}

struct SettingsUpdateRequest: Codable, Sendable {
    var llmBackend: String?
    var ollamaModel: String?
    var openaiModel: String?
    var openaiApiKey: String?
    var azureOpenaiEndpoint: String?
    var azureOpenaiDeployment: String?
    var azureOpenaiApiKey: String?
    var captureEnabled: Bool?
    var captureIntervalSeconds: Int?
    var checkinEnabled: Bool?
    var checkinTimes: [String]?
    var checkinJitterMinutes: Int?
    var learningEnabled: Bool?
    var learningIntervalMinutes: Int?
    var conversationInactivityTimeoutSeconds: Int?
    var conversationAutoCloseMinutes: Int?
    var conversationSummaryEnabled: Bool?
    var goalCheckIntervalSeconds: Double?
    var projectsDirectory: String?
    var toolsEnabled: Bool?
    var toolsMaxIterations: Int?
    var mcpEnabled: Bool?
    var similarityDeduplicationThreshold: Double?
    var similaritySearchRecallThreshold: Double?
    var similarityClusteringThreshold: Double?
    var memoryShortTermRetentionDays: Int?
    var memoryLongTermRetentionDays: Int?
    var goalWorkerEnabled: Bool?
    var goalWorkerAutonomous: Bool?
    var goalWorkerMaxConcurrent: Int?
    var visionBackend: String?
    var visionOllamaModel: String?
    var localeOverride: String?

    enum CodingKeys: String, CodingKey {
        case llmBackend = "llm_backend"
        case ollamaModel = "ollama_model"
        case openaiModel = "openai_model"
        case openaiApiKey = "openai_api_key"
        case azureOpenaiEndpoint = "azure_openai_endpoint"
        case azureOpenaiDeployment = "azure_openai_deployment"
        case azureOpenaiApiKey = "azure_openai_api_key"
        case captureEnabled = "capture_enabled"
        case captureIntervalSeconds = "capture_interval_seconds"
        case checkinEnabled = "checkin_enabled"
        case checkinTimes = "checkin_times"
        case checkinJitterMinutes = "checkin_jitter_minutes"
        case learningEnabled = "learning_enabled"
        case learningIntervalMinutes = "learning_interval_minutes"
        case conversationInactivityTimeoutSeconds = "conversation_inactivity_timeout_seconds"
        case conversationAutoCloseMinutes = "conversation_auto_close_minutes"
        case conversationSummaryEnabled = "conversation_summary_enabled"
        case goalCheckIntervalSeconds = "goal_check_interval_seconds"
        case projectsDirectory = "projects_directory"
        case toolsEnabled = "tools_enabled"
        case toolsMaxIterations = "tools_max_iterations"
        case mcpEnabled = "mcp_enabled"
        case similarityDeduplicationThreshold = "similarity_deduplication_threshold"
        case similaritySearchRecallThreshold = "similarity_search_recall_threshold"
        case similarityClusteringThreshold = "similarity_clustering_threshold"
        case memoryShortTermRetentionDays = "memory_short_term_retention_days"
        case memoryLongTermRetentionDays = "memory_long_term_retention_days"
        case goalWorkerEnabled = "goal_worker_enabled"
        case goalWorkerAutonomous = "goal_worker_autonomous"
        case goalWorkerMaxConcurrent = "goal_worker_max_concurrent"
        case visionBackend = "vision_backend"
        case visionOllamaModel = "vision_ollama_model"
        case localeOverride = "locale_override"
    }
}

struct SettingsUpdateResponse: Codable, Sendable {
    let message: String
    let appliedFields: [String]
    let restartRequiredFields: [String]

    enum CodingKeys: String, CodingKey {
        case message
        case appliedFields = "applied_fields"
        case restartRequiredFields = "restart_required_fields"
    }
}

// MARK: - Goal Worker

struct GoalWorkerStatusResponse: Codable, Sendable {
    let enabled: Bool
    let maxConcurrent: Int
    let activeGoalsCount: Int
    let pendingApprovalCount: Int

    enum CodingKeys: String, CodingKey {
        case enabled
        case maxConcurrent = "max_concurrent"
        case activeGoalsCount = "active_goals_count"
        case pendingApprovalCount = "pending_approval_count"
    }
}

// MARK: - Models (LLM Model Management)

struct ModelInfo: Identifiable, Codable, Sendable {
    var id: String {
        self.name
    }

    let name: String
    let sizeBytes: Int
    let modifiedAt: String

    enum CodingKeys: String, CodingKey {
        case name
        case sizeBytes = "size_bytes"
        case modifiedAt = "modified_at"
    }
}

struct ModelsListResponse: Codable, Sendable {
    let backend: String
    let models: [ModelInfo]
    let supportsPull: Bool
    /// Set when Ollama is unavailable; models will be empty in that case.
    let ollamaError: String?

    enum CodingKeys: String, CodingKey {
        case backend, models
        case supportsPull = "supports_pull"
        case ollamaError = "ollama_error"
    }
}

// MARK: - Message / Health

struct SendMessageRequest: Codable, Sendable {
    let content: String
}

struct HealthResponse: Codable, Sendable {
    let status: String
    let setupCompleted: Bool?

    enum CodingKeys: String, CodingKey {
        case status
        case setupCompleted = "setup_completed"
    }
}

// MARK: - Onboarding

struct OnboardingStatusResponse: Codable, Sendable {
    let needsOnboarding: Bool
    let complete: Bool

    enum CodingKeys: String, CodingKey {
        case needsOnboarding = "needs_onboarding"
        case complete
    }
}

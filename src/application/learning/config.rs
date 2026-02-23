//! Configuration for the learning system.

/// Thresholds for similarity operations.
#[derive(Debug, Clone)]
pub struct SimilarityConfig {
    /// Strict duplicate detection.
    pub deduplication_threshold: f64,
    /// Include related results.
    pub search_recall_threshold: f64,
    /// Group similar memories.
    pub clustering_threshold: f64,
}

impl Default for SimilarityConfig {
    fn default() -> Self {
        Self {
            deduplication_threshold: 0.85,
            search_recall_threshold: 0.60,
            clustering_threshold: 0.80,
        }
    }
}

/// Configuration for the LearningLoop.
#[derive(Debug, Clone)]
pub struct LearningConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub min_context_items: u32,
    pub max_context_per_cycle: u32,
    pub max_memories_per_cycle: u32,
    pub max_goals_per_cycle: u32,
    pub max_memories_per_consolidation: u32,
    pub daily_consolidation_hour: u32,
    pub similarity: SimilarityConfig,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 30,
            min_context_items: 5,
            max_context_per_cycle: 50,
            max_memories_per_cycle: 10,
            max_goals_per_cycle: 3,
            max_memories_per_consolidation: 1000,
            daily_consolidation_hour: 3,
            similarity: SimilarityConfig::default(),
        }
    }
}

impl LearningConfig {
    pub fn from_app_config(config: &crate::config::Config) -> Self {
        Self {
            enabled: config.learning_enabled,
            interval_minutes: config.learning_interval_minutes,
            min_context_items: config.learning_min_context_items,
            max_context_per_cycle: config.learning_max_context_per_cycle,
            max_memories_per_cycle: config.learning_max_memories_per_cycle,
            max_goals_per_cycle: config.learning_max_goals_per_cycle,
            max_memories_per_consolidation: config.learning_max_memories_per_consolidation,
            daily_consolidation_hour: config.daily_consolidation_hour,
            similarity: SimilarityConfig {
                deduplication_threshold: config.similarity_deduplication_threshold,
                search_recall_threshold: config.similarity_search_recall_threshold,
                clustering_threshold: config.similarity_clustering_threshold,
            },
        }
    }
}

/// How long different types of data are kept.
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    pub raw_context_days: u32,
    pub short_term_memory_days: u32,
    pub long_term_memory_days: u32,
    pub goal_retention_days: u32,
    pub pruning_enabled: bool,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            raw_context_days: 7,
            short_term_memory_days: 30,
            long_term_memory_days: 90,
            goal_retention_days: 30,
            pruning_enabled: true,
        }
    }
}

impl RetentionConfig {
    pub fn from_app_config(config: &crate::config::Config) -> Self {
        Self {
            raw_context_days: config.memory_raw_context_retention_days,
            short_term_memory_days: config.memory_short_term_retention_days,
            long_term_memory_days: config.memory_long_term_retention_days,
            goal_retention_days: config.goal_retention_days,
            pruning_enabled: config.memory_pruning_enabled,
        }
    }
}

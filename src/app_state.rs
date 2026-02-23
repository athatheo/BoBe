use std::sync::Arc;
use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;

use crate::adapters::capture::ScreenCapture;
use crate::adapters::llm::ollama_manager::OllamaManager;
use crate::adapters::sse::connection_manager::SseConnectionManager;
use crate::adapters::sse::event_queue::EventQueue;
use crate::composition::config_manager::ConfigManager;
use crate::config::Config;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::llm::LlmProvider;
use crate::ports::repos::soul_repo::SoulRepository;
use crate::ports::repos::user_profile_repo::UserProfileRepository;

/// Shared application state passed through Axum extractors.
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<ArcSwap<Config>>,
    pub http_client: Client,
    pub event_queue: Arc<EventQueue>,
    pub connection_manager: Arc<SseConnectionManager>,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub embedding_provider: Arc<dyn EmbeddingProvider>,
    pub soul_repo: Arc<dyn SoulRepository>,
    pub user_profile_repo: Arc<dyn UserProfileRepository>,
    pub screen_capture: Arc<ScreenCapture>,
    pub ollama_manager: Arc<OllamaManager>,
    pub config_manager: Arc<ConfigManager>,
}

impl AppState {
    pub fn config(&self) -> arc_swap::Guard<Arc<Config>> {
        self.config.load()
    }
}

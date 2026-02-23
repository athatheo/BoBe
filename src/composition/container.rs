use std::sync::Arc;

use arc_swap::ArcSwap;
use reqwest::Client;
use sqlx::sqlite::SqlitePool;

use crate::adapters::capture::ScreenCapture;
use crate::adapters::embedding::LocalEmbeddingProvider;
use crate::adapters::llm::factory::LlmProviderFactory;
use crate::adapters::llm::ollama_manager::OllamaManager;
use crate::adapters::network::MdnsAnnouncer;
use crate::adapters::persistence::repos::soul_repo::SqliteSoulRepo;
use crate::adapters::persistence::repos::user_profile_repo::SqliteUserProfileRepo;
use crate::adapters::sse::connection_manager::SseConnectionManager;
use crate::adapters::sse::event_queue::EventQueue;
use crate::config::Config;
use crate::ports::embedding::EmbeddingProvider;
use crate::ports::llm::LlmProvider;
use crate::ports::repos::soul_repo::SoulRepository;
use crate::ports::repos::user_profile_repo::UserProfileRepository;

use super::config_manager::ConfigManager;

/// Holds all Arc<dyn Trait> dependencies for the application.
///
/// This is the composition root — the only place that knows all
/// concrete types. Everything else works with trait objects.
pub struct Container {
    pub db: SqlitePool,
    pub config: Arc<ArcSwap<Config>>,
    pub http_client: Client,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub embedding_provider: Arc<dyn EmbeddingProvider>,
    pub event_queue: Arc<EventQueue>,
    pub connection_manager: Arc<SseConnectionManager>,
    pub soul_repo: Arc<dyn SoulRepository>,
    pub user_profile_repo: Arc<dyn UserProfileRepository>,
    pub screen_capture: Arc<ScreenCapture>,
    pub ollama_manager: Arc<OllamaManager>,
    pub mdns_announcer: Arc<MdnsAnnouncer>,
    pub config_manager: Arc<ConfigManager>,
}

impl Container {
    /// Build the container from a config and database pool.
    ///
    /// This wires all concrete implementations to trait objects.
    pub fn build(
        config: Config,
        pool: SqlitePool,
    ) -> Result<Self, crate::error::AppError> {
        let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| crate::error::AppError::Internal(format!("HTTP client build failed: {e}")))?;

        // LLM provider
        let llm_factory = LlmProviderFactory::new(http_client.clone(), config.clone());
        let llm_provider = llm_factory.create(&config.llm_backend)?;

        // Embedding provider (calls Ollama /api/embed)
        let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(
            LocalEmbeddingProvider::new(
                http_client.clone(),
                &config.ollama_url,
                "nomic-embed-text",
                config.embedding_dimension,
            ),
        );

        // SSE
        let event_queue = Arc::new(EventQueue::new(100));
        let connection_manager = Arc::new(SseConnectionManager::new());

        // Repos
        let soul_repo: Arc<dyn SoulRepository> =
            Arc::new(SqliteSoulRepo::new(pool.clone()));
        let user_profile_repo: Arc<dyn UserProfileRepository> =
            Arc::new(SqliteUserProfileRepo::new(pool.clone()));

        // Capture
        let screen_capture = Arc::new(ScreenCapture::new());

        // Ollama manager
        let ollama_manager = Arc::new(OllamaManager::new(
            http_client.clone(),
            &config.ollama_url,
            &config.ollama_model,
            config.ollama_auto_start,
            config.ollama_auto_pull,
            config.ollama_binary_path.clone(),
        ));

        // Network
        let mdns_announcer = Arc::new(MdnsAnnouncer::new(
            config.port,
            config.mdns_enabled && config.host == "0.0.0.0",
        ));

        // Config manager
        let config_manager = Arc::new(ConfigManager::new(config_arc.clone()));

        Ok(Self {
            db: pool,
            config: config_arc,
            http_client,
            llm_provider,
            embedding_provider,
            event_queue,
            connection_manager,
            soul_repo,
            user_profile_repo,
            screen_capture,
            ollama_manager,
            mdns_announcer,
            config_manager,
        })
    }
}

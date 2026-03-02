//! Infrastructure layer — providers, transports, and managed processes.
//!
//! Everything here is stateless with respect to domain data; it only
//! needs `Config` and produces the provider/transport handles that the
//! rest of bootstrap consumes.

use std::sync::Arc;

use arc_swap::ArcSwap;
use reqwest::Client;
use tracing::{error, info, warn};

use crate::binary_manager::BinaryManager;
use crate::config::{Config, LlmBackend};
use crate::error::AppError;
use crate::llm::factory::LlmProviderFactory;
use crate::llm::ollama_manager::OllamaManager;
use crate::llm::swappable::{SwappableEmbeddingProvider, SwappableLlmProvider};
use crate::llm::{EmbeddingProvider, LlmProvider};
use crate::util::network::MdnsAnnouncer;
use crate::util::sse::connection_manager::SseConnectionManager;
use crate::util::sse::event_queue::EventQueue;

/// Shared infrastructure handles needed across all bootstrap phases.
pub struct Infrastructure {
    pub config_arc: Arc<ArcSwap<Config>>,
    pub http_client: Client,
    pub llm_provider: Arc<dyn LlmProvider>,
    pub vision_llm_provider: Option<Arc<dyn LlmProvider>>,
    pub embedding_provider: Arc<dyn EmbeddingProvider>,
    pub llm_swap_handle: Arc<ArcSwap<Arc<dyn LlmProvider>>>,
    pub embedding_swap_handle: Arc<ArcSwap<Arc<dyn EmbeddingProvider>>>,
    pub llm_factory: Arc<LlmProviderFactory>,
    pub event_queue: Arc<EventQueue>,
    pub connection_manager: Arc<SseConnectionManager>,
    pub ollama_manager: Arc<OllamaManager>,
    pub binary_manager: Arc<BinaryManager>,
    pub mdns_announcer: Arc<MdnsAnnouncer>,
}

impl Infrastructure {
    /// Build all infrastructure from a validated config.
    pub fn build(config: &Config) -> Result<Self, AppError> {
        let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| AppError::Internal(format!("HTTP client: {e}")))?;

        // LLM
        let llm_factory = Arc::new(LlmProviderFactory::new(
            http_client.clone(),
            config_arc.clone(),
        ));
        let (swappable, llm_swap_handle) =
            SwappableLlmProvider::new(llm_factory.create(config.llm.backend)?);
        let llm_provider: Arc<dyn LlmProvider> = Arc::new(swappable);

        let vision_llm_provider = match config.vision.backend {
            LlmBackend::None => None,
            backend => Some(llm_factory.create_vision(backend)?),
        };

        // Embedding
        let (swappable_embed, embedding_swap_handle) =
            SwappableEmbeddingProvider::new(llm_factory.create_embedding()?);
        let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(swappable_embed);

        // SSE
        let event_queue = Arc::new(EventQueue::new(100));
        let connection_manager =
            Arc::new(SseConnectionManager::new(event_queue.clone(), None, None));

        // Ollama
        let ollama_manager = Arc::new(OllamaManager::new(
            http_client.clone(),
            &config.ollama.url,
            &config.ollama.model,
            config.ollama.auto_start,
            config.ollama.auto_pull,
            config.ollama.binary_path.clone(),
        ));

        // Binary manager
        let binary_manager = Arc::new(BinaryManager::new(
            &config.resolved_data_dir(),
            Arc::new(http_client.clone()),
        ));

        // mDNS
        let mdns_announcer = Arc::new(MdnsAnnouncer::new(
            config.server.port,
            config.server.mdns_enabled && config.server.host == "0.0.0.0",
        ));

        Ok(Self {
            config_arc,
            http_client,
            llm_provider,
            vision_llm_provider,
            embedding_provider,
            llm_swap_handle,
            embedding_swap_handle,
            llm_factory,
            event_queue,
            connection_manager,
            ollama_manager,
            binary_manager,
            mdns_announcer,
        })
    }
}

/// Best-effort Ollama startup — never fails the bootstrap.
pub async fn ensure_ollama_ready(config: &Config, manager: &OllamaManager) {
    let needs_ollama =
        config.llm.backend == LlmBackend::Ollama || config.vision.backend == LlmBackend::Ollama;

    if !needs_ollama {
        return;
    }

    match manager.ensure_running().await {
        Ok(()) => info!(model = %config.ollama.model, "ollama.ready"),
        Err(e) => {
            error!(error = %e, "ollama.startup_failed");
            warn!("Continuing without Ollama — LLM calls will fail until it's available");
        }
    }

    if config.vision.backend == LlmBackend::Ollama {
        match manager.ensure_model(&config.vision.ollama_model).await {
            Ok(true) => info!(model = %config.vision.ollama_model, "ollama.vision_ready"),
            Ok(false) => {
                warn!(model = %config.vision.ollama_model, "ollama.vision_model_unavailable");
            }
            Err(e) => warn!(error = %e, "ollama.vision_model_check_failed"),
        }
    }
}

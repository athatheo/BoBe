//! Cost-aware picker for Copilot models. Walks the user's live `list_models()` and
//! returns the cheapest one matching a task's capability requirements. Cached for
//! `CACHE_TTL`; falls back to `"auto"` (which every account has) on any failure.

use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use github_copilot_sdk::types::Model;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::copilot::client::ClientHandle;
use crate::copilot::types::WorkerClass;
use crate::error::AppError;

const CACHE_TTL: Duration = Duration::from_secs(300);
const FALLBACK_MODEL: &str = "auto";

/// What a task needs from a model; the resolver filters candidates by this.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskRequirements {
    pub(crate) needs_vision: bool,
}

impl TaskRequirements {
    pub(crate) const fn from_class(class: WorkerClass) -> Self {
        match class {
            WorkerClass::Vision => Self { needs_vision: true },
            _ => Self { needs_vision: false },
        }
    }
}

pub(crate) struct ModelResolver {
    client: Arc<ClientHandle>,
    cache: Mutex<Option<CacheEntry>>,
}

struct CacheEntry {
    models: Vec<Model>,
    fetched_at: Instant,
}

impl ModelResolver {
    pub(crate) fn new(client: Arc<ClientHandle>) -> Arc<Self> {
        Arc::new(Self {
            client,
            cache: Mutex::new(None),
        })
    }

    /// Picks the cheapest available model that satisfies `req`. Returns `"auto"` if
    /// the SDK call fails or no candidate matches — callers should NOT treat that as an error.
    pub(crate) async fn pick(&self, req: TaskRequirements) -> String {
        let models = match self.load_models().await {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "model_resolver.list_failed_using_fallback");
                return FALLBACK_MODEL.to_string();
            }
        };

        let mut candidates: Vec<&Model> = models
            .iter()
            .filter(|m| !is_disabled(m))
            .filter(|m| !req.needs_vision || supports_vision(m))
            // "auto" has no billing info and is a router, not a leaf — skip during sort,
            // but it remains as our final fallback below.
            .filter(|m| m.id != FALLBACK_MODEL)
            .collect();

        candidates.sort_by(|a, b| compare_cost(a, b));

        if let Some(pick) = candidates.first() {
            debug!(
                picked = %pick.id,
                multiplier = ?pick.billing.as_ref().map(|b| b.multiplier),
                needs_vision = req.needs_vision,
                "model_resolver.pick"
            );
            return pick.id.clone();
        }

        debug!(needs_vision = req.needs_vision, "model_resolver.no_match_using_auto");
        FALLBACK_MODEL.to_string()
    }

    async fn load_models(&self) -> Result<Vec<Model>, AppError> {
        {
            let guard = self.cache.lock().await;
            if let Some(entry) = guard.as_ref()
                && entry.fetched_at.elapsed() < CACHE_TTL
            {
                return Ok(entry.models.clone());
            }
        }

        let client = self.client.ensure_started().await?;
        let models = client
            .list_models()
            .await
            .map_err(|e| AppError::Internal(format!("ModelResolver.list_models: {e}")))?;

        let mut guard = self.cache.lock().await;
        *guard = Some(CacheEntry {
            models: models.clone(),
            fetched_at: Instant::now(),
        });
        Ok(models)
    }

    pub(crate) async fn invalidate(&self) {
        let mut guard = self.cache.lock().await;
        *guard = None;
    }
}

fn supports_vision(m: &Model) -> bool {
    m.capabilities
        .supports
        .as_ref()
        .and_then(|s| s.vision)
        .unwrap_or(false)
}

fn is_disabled(m: &Model) -> bool {
    matches!(
        m.policy.as_ref().map(|p| p.state.as_str()),
        Some("disabled") | Some("unconfigured")
    )
}

/// Multiplier ascending; missing billing sorts to the end (treat as "unknown cost = avoid").
fn compare_cost(a: &Model, b: &Model) -> Ordering {
    let am = a.billing.as_ref().map(|b| b.multiplier).unwrap_or(f64::INFINITY);
    let bm = b.billing.as_ref().map(|b| b.multiplier).unwrap_or(f64::INFINITY);
    am.partial_cmp(&bm).unwrap_or(Ordering::Equal)
}

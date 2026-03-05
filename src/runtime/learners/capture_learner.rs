//! Processes screen captures: vision LLM analysis, category inference, embedding, and visual memory diary.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use base64::Engine;
use chrono::{DateTime, Timelike, Utc};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::db::MemoryRepository;
use crate::db::ObservationRepository;
use crate::error::AppError;
use crate::llm::EmbeddingProvider;
use crate::llm::LlmProvider;
use crate::models::memory::Memory;
use crate::models::observation::Observation;
use crate::models::types::{MemorySource, MemoryType, ObservationSource};
use crate::runtime::learners::types::{
    LearnerError, LearnerObservation, LearnerObservationSource, LearnerResult,
};
use crate::runtime::prompts::capture::{
    ALLOWED_CATEGORIES, VisionAnalysisPrompt, VisualMemoryConsolidationPrompt,
};

static CATEGORY_KEYWORDS: &[(&str, &[&str])] = &[
    (
        "coding",
        &[
            "vscode",
            "vs code",
            "visual studio",
            "intellij",
            "pycharm",
            "neovim",
            "vim",
            "emacs",
            "sublime",
            "cursor",
            ".py",
            ".ts",
            ".js",
            ".rs",
            ".go",
            "def ",
            "function ",
            "class ",
            "import ",
        ],
    ),
    (
        "terminal",
        &[
            "terminal",
            "iterm",
            "wezterm",
            "alacritty",
            "kitty",
            "command line",
            "shell",
            "bash",
            "zsh",
            "fish",
            "$ ",
            "❯",
        ],
    ),
    (
        "browsing",
        &[
            "chrome", "firefox", "safari", "brave", "edge", "browser", "http://", "https://",
            "www.",
        ],
    ),
    (
        "documentation",
        &[
            "docs",
            "documentation",
            "readme",
            "wiki",
            "notion",
            "confluence",
            "man page",
            "reference",
        ],
    ),
    (
        "communication",
        &[
            "slack", "discord", "teams", "zoom", "meet", "email", "gmail", "outlook", "messages",
            "chat",
        ],
    ),
    (
        "design",
        &[
            "figma",
            "sketch",
            "photoshop",
            "illustrator",
            "canva",
            "design",
        ],
    ),
    (
        "media",
        &[
            "youtube", "spotify", "netflix", "video", "music", "vlc", "podcast",
        ],
    ),
];

pub(crate) struct CaptureLearner {
    llm: Arc<dyn LlmProvider>,
    vision_llm: Option<Arc<dyn LlmProvider>>,
    embedding: Arc<dyn EmbeddingProvider>,
    observation_repo: Arc<dyn ObservationRepository>,
    memory_repo: Arc<dyn MemoryRepository>,
    config: Arc<ArcSwap<Config>>,
}

impl CaptureLearner {
    pub(crate) fn new(
        llm: Arc<dyn LlmProvider>,
        embedding: Arc<dyn EmbeddingProvider>,
        observation_repo: Arc<dyn ObservationRepository>,
        memory_repo: Arc<dyn MemoryRepository>,
        vision_llm: Option<Arc<dyn LlmProvider>>,
        config: Arc<ArcSwap<Config>>,
    ) -> Self {
        Self {
            llm,
            vision_llm,
            embedding,
            observation_repo,
            memory_repo,
            config,
        }
    }

    fn effective_vision_llm(&self) -> Option<&Arc<dyn LlmProvider>> {
        if let Some(ref v) = self.vision_llm
            && v.supports_vision()
        {
            return Some(v);
        }
        if self.llm.supports_vision() {
            return Some(&self.llm);
        }
        None
    }

    pub(crate) async fn learn(
        &self,
        observation: &LearnerObservation,
    ) -> Result<LearnerResult, LearnerError> {
        if observation.source != LearnerObservationSource::Capture {
            return Err(LearnerError::WrongSource {
                expected: "capture".into(),
                got: observation.source.as_str().into(),
            });
        }

        let screenshot = observation
            .screenshot
            .as_ref()
            .ok_or_else(|| LearnerError::MissingData("screenshot data required".into()))?;

        info!(
            window = ?observation.active_window,
            img_kb = screenshot.len() / 1024,
            "capture_learner.started"
        );

        let analysis = self
            .analyze(screenshot, observation.active_window.as_deref())
            .await;

        let description = analysis.description;
        let category = analysis.category;

        let embedding_vec = self
            .embedding
            .embed(&description)
            .await
            .map_err(|e| LearnerError::Embedding(e.to_string()))?;

        let mut obs = Observation::new(
            ObservationSource::Screen,
            description.clone(),
            category.clone(),
        );
        obs.embedding = Some(
            serde_json::to_string(&embedding_vec)
                .map_err(|e| LearnerError::Storage(e.to_string()))?,
        );
        let summary = Self::create_summary(&description, observation.active_window.as_deref());
        let mut meta = HashMap::new();
        if let Some(ref w) = observation.active_window {
            meta.insert(
                "active_window".to_string(),
                serde_json::Value::String(w.clone()),
            );
        }
        meta.insert("summary".to_string(), serde_json::Value::String(summary));
        obs.metadata =
            Some(serde_json::to_string(&meta).map_err(|e| LearnerError::Storage(e.to_string()))?);

        let stored = self.observation_repo.save(&obs).await?;

        info!(
            item_id = %stored.id,
            category = %category,
            "capture_learner.stored"
        );

        if let Err(e) = self
            .update_visual_memory(&description, &stored.id.to_string())
            .await
        {
            warn!(error = %e, "capture_learner.visual_memory_update_failed");
        }

        Ok(LearnerResult::Stored {
            observation_id: stored.id,
        })
    }

    async fn analyze(&self, screenshot: &[u8], active_window: Option<&str>) -> AnalysisResult {
        let Some(vision_llm) = self.effective_vision_llm() else {
            debug!("capture_learner.vision_skipped");
            return Self::degraded_analysis(active_window);
        };

        match self.call_vision_llm(screenshot, vision_llm.as_ref()).await {
            Ok(raw_output) => {
                let category = Self::infer_category(&raw_output, active_window);
                debug!(category = %category, chars = raw_output.len(), "capture_learner.vision_complete");
                AnalysisResult {
                    description: raw_output,
                    category,
                }
            }
            Err(e) => {
                warn!(error = %e, "capture_learner.vision_failed");
                Self::degraded_analysis(active_window)
            }
        }
    }

    async fn call_vision_llm(
        &self,
        screenshot: &[u8],
        vision_llm: &dyn LlmProvider,
    ) -> Result<String, AppError> {
        let image_b64 = base64::engine::general_purpose::STANDARD.encode(screenshot);
        let image_url = format!("data:image/png;base64,{image_b64}");
        let locale = self.config.load().effective_locale();

        let messages = VisionAnalysisPrompt::messages(&image_url, Some(&locale));
        let config = VisionAnalysisPrompt::config();

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(240),
            vision_llm.complete(
                &messages,
                None,
                config.response_format.as_ref(),
                config.temperature,
                config.max_tokens,
            ),
        )
        .await
        .map_err(|_| AppError::LlmTimeout("vision analysis timed out".into()))??;

        let content = response.message.content.text_or_empty().to_string();
        if content.is_empty() {
            return Err(AppError::Llm("Vision LLM returned empty response".into()));
        }

        Ok(content)
    }

    fn infer_category(description: &str, active_window: Option<&str>) -> String {
        let text = format!("{} {}", description, active_window.unwrap_or("")).to_lowercase();

        let mut best_category = "other";
        let mut best_score = 0usize;

        for &(category, keywords) in CATEGORY_KEYWORDS {
            let score = keywords.iter().filter(|kw| text.contains(*kw)).count();
            if score > best_score {
                best_score = score;
                best_category = category;
            }
        }

        if !ALLOWED_CATEGORIES.contains(&best_category) {
            best_category = "other";
        }

        best_category.to_owned()
    }

    async fn update_visual_memory(
        &self,
        description: &str,
        observation_id: &str,
    ) -> Result<(), LearnerError> {
        let now = Utc::now();
        let timestamp = now.format("%H:%M").to_string();
        let period = if now.hour() < 12 { "AM" } else { "PM" };
        let date_str = now.format("%Y-%m-%d").to_string();
        let header = format!("# Visual Memory {date_str} {period}");

        let window_start_hour = if now.hour() < 12 { 0 } else { 12 };
        let window_start = now
            .date_naive()
            .and_hms_opt(window_start_hour, 0, 0)
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));

        let existing_content = self
            .find_visual_diary_memory(window_start)
            .await
            .map(|m| m.content)
            .unwrap_or_default();
        let llm_context = Self::tail_lines(&existing_content, 30);
        let locale = self.config.load().effective_locale();

        let obs_id_short = &observation_id[..observation_id.len().min(8)];
        let messages = VisualMemoryConsolidationPrompt::messages(
            &llm_context,
            description,
            &timestamp,
            obs_id_short,
            Some(&locale),
        );
        let config = VisualMemoryConsolidationPrompt::config();

        let response = self
            .llm
            .complete(
                &messages,
                None,
                config.response_format.as_ref(),
                config.temperature,
                config.max_tokens,
            )
            .await
            .map_err(|e| LearnerError::Llm(e.to_string()))?;

        let updated_diary = response.message.content.text_or_empty().trim().to_string();
        if updated_diary.is_empty() {
            return Ok(());
        }

        let updated_diary = if updated_diary.starts_with('#') {
            updated_diary
        } else {
            format!("{header}\n\n{updated_diary}")
        };

        let existing = self.find_visual_diary_memory(window_start).await;

        if let Some(mut diary) = existing {
            diary.content = updated_diary;
            diary.updated_at = Utc::now();
            self.memory_repo
                .update(diary.id, Some(&diary.content), None, None)
                .await?;
            debug!("capture_learner.visual_memory_updated");
        } else {
            let new_memory = Memory::new(
                updated_diary,
                MemoryType::ShortTerm,
                MemorySource::VisualDiary,
                "observation".into(),
            );
            self.memory_repo.save(&new_memory).await?;
            debug!(period = %format!("{date_str} {period}"), "capture_learner.visual_memory_created");
        }

        Ok(())
    }

    async fn find_visual_diary_memory(
        &self,
        window_start: Option<DateTime<Utc>>,
    ) -> Option<Memory> {
        let memories = self
            .memory_repo
            .find_all(
                Some(MemoryType::ShortTerm.as_str()),
                Some("observation"),
                Some(MemorySource::VisualDiary.as_str()),
                false,
                5,
                0,
            )
            .await
            .ok()?;

        let ws = window_start?;
        memories.0.into_iter().find(|m| m.created_at >= ws)
    }

    fn tail_lines(text: &str, max_lines: usize) -> String {
        if text.is_empty() {
            return text.to_owned();
        }
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() <= max_lines {
            return text.to_owned();
        }
        let header = if lines[0].starts_with('#') {
            lines[0]
        } else {
            ""
        };
        let tail: Vec<&str> = lines[lines.len() - max_lines..].to_vec();
        if header.is_empty() {
            tail.join("\n")
        } else {
            format!("{header}\n...\n{}", tail.join("\n"))
        }
    }

    fn degraded_analysis(active_window: Option<&str>) -> AnalysisResult {
        let description = match active_window {
            Some(w) => format!("User is using {w}"),
            None => "Unable to determine screen content".into(),
        };
        AnalysisResult {
            description,
            category: "other".into(),
        }
    }

    fn create_summary(description: &str, active_window: Option<&str>) -> String {
        let app_prefix = match active_window {
            Some(w) => format!("[{w}] "),
            None => String::new(),
        };
        let max_len = 200;
        let summary = if description.len() <= max_len {
            description.to_owned()
        } else {
            format!(
                "{}...",
                crate::util::text::truncate_str(description, max_len - 3)
            )
        };
        format!("{app_prefix}{summary}")
    }
}

struct AnalysisResult {
    description: String,
    category: String,
}

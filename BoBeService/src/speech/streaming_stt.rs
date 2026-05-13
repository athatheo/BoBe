//! Streaming STT — fed every Opus-decoded mic frame, emits partial text as
//! the model converges, finalizes when the voice handler signals
//! end-of-speech. Replaces the old segment-based SttEngine + Moonshine.
//!
//! Why streaming: partial transcripts unlock backchannel suppression
//! (MinWords gate), cancel-phrase detection ("stop"/"nevermind" bypass
//! the LLM), and future speculative LLM kick-off before the user
//! finishes. Moonshine couldn't do any of that — it's offline-on-segment.
//!
//! Backed by sherpa-onnx's `OnlineRecognizer` driving a Zipformer
//! transducer (English-only int8). The wrapped `OnlineStream` is mutex-
//! guarded because the voice WS handler feeds it on the main rx loop
//! while a separate `pop_partial` may fire from the per-turn task.

use std::path::Path;
use std::sync::Mutex;

use sherpa_onnx::{
    OnlineModelConfig, OnlineRecognizer, OnlineRecognizerConfig, OnlineStream,
    OnlineTransducerModelConfig,
};

use crate::error::AppError;

const SAMPLE_RATE_HZ: i32 = 16_000;

/// Streaming recognizer wired into the voice WS Binary arm.
pub(crate) trait StreamingSttEngine: Send + Sync {
    /// Push one chunk of 16kHz mono f32 PCM from the mic. Drives decoding
    /// internally so subsequent `pop_partial` calls see fresh state.
    fn accept_audio(&self, samples: &[f32]) -> Result<(), AppError>;

    /// Return the latest decoded text iff it changed since the last call.
    /// `None` means "no new partial worth emitting." Implementations
    /// dedupe so the WS doesn't spam clients with identical partials.
    fn pop_partial(&self) -> Option<String>;

    /// Drain remaining frames + return the final transcript text. Used by
    /// the voice handler when Silero signals end-of-speech. The stream is
    /// reset afterward so the next turn starts clean.
    fn commit_final(&self) -> Result<String, AppError>;

    /// Force a reset between turns (e.g. on barge-in cleanup) without
    /// reading a final transcript.
    fn reset(&self);
}

/// sherpa-onnx Zipformer (streaming, EN-only int8). Loaded from a model
/// directory containing the standard encoder/decoder/joiner ONNX trio +
/// tokens.txt.
pub(crate) struct LocalZipformerStt {
    recognizer: OnlineRecognizer,
    /// `OnlineStream` is `Send + Sync` per the upstream impl but its API
    /// mutates internal C state through `&self` — wrap in a Mutex so the
    /// voice handler's main rx loop (accept_audio) and the per-turn task
    /// (pop_partial / commit_final) don't race.
    stream: Mutex<OnlineStream>,
    /// Last partial text emitted; pop_partial returns Some only when
    /// the underlying recognizer text changes.
    last_partial: Mutex<String>,
}

impl LocalZipformerStt {
    pub(crate) fn load(
        model_dir: &Path,
        provider: &str,
        num_threads: i32,
    ) -> Result<Self, AppError> {
        let encoder = model_dir.join("encoder.onnx");
        let decoder = model_dir.join("decoder.onnx");
        let joiner = model_dir.join("joiner.onnx");
        let tokens = model_dir.join("tokens.txt");
        for path in [&encoder, &decoder, &joiner, &tokens] {
            if !path.exists() {
                return Err(AppError::Internal(format!(
                    "streaming-stt missing file: {}",
                    path.display()
                )));
            }
        }
        let config = OnlineRecognizerConfig {
            model_config: OnlineModelConfig {
                transducer: OnlineTransducerModelConfig {
                    encoder: Some(encoder.to_string_lossy().into_owned()),
                    decoder: Some(decoder.to_string_lossy().into_owned()),
                    joiner: Some(joiner.to_string_lossy().into_owned()),
                },
                tokens: Some(tokens.to_string_lossy().into_owned()),
                num_threads,
                provider: Some(provider.to_string()),
                ..Default::default()
            },
            enable_endpoint: false, // Silero owns endpoint detection.
            decoding_method: Some("greedy_search".into()),
            ..Default::default()
        };
        let recognizer = OnlineRecognizer::create(&config)
            .ok_or_else(|| AppError::Internal("streaming-stt OnlineRecognizer::create returned None".into()))?;
        let stream = recognizer.create_stream();
        Ok(Self {
            recognizer,
            stream: Mutex::new(stream),
            last_partial: Mutex::new(String::new()),
        })
    }

    fn current_text(&self) -> String {
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.recognizer
            .get_result(&stream)
            .map(|r| r.text)
            .unwrap_or_default()
    }
}

impl StreamingSttEngine for LocalZipformerStt {
    fn accept_audio(&self, samples: &[f32]) -> Result<(), AppError> {
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stream.accept_waveform(SAMPLE_RATE_HZ, samples);
        // Drive the model forward until it's drained whatever we just fed.
        while self.recognizer.is_ready(&stream) {
            self.recognizer.decode(&stream);
        }
        Ok(())
    }

    fn pop_partial(&self) -> Option<String> {
        let text = self.current_text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut last = self
            .last_partial
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if trimmed == last.as_str() {
            return None;
        }
        *last = trimmed.to_string();
        Some(trimmed.to_string())
    }

    fn commit_final(&self) -> Result<String, AppError> {
        let text = self.current_text();
        let final_text = text.trim().to_string();
        // Reset the underlying stream so the next turn starts clean.
        self.reset();
        Ok(final_text)
    }

    fn reset(&self) {
        let mut stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.recognizer.reset(&stream);
        // A fresh stream is cheaper than relying on reset's internal state
        // when sherpa-onnx is across a turn boundary.
        *stream = self.recognizer.create_stream();
        *self
            .last_partial
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = String::new();
    }
}

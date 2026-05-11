//! Local STT via sherpa-onnx Moonshine (English-only, offline).
//!
//! Future: swap to streaming-zipformer for true mid-utterance partials.

use std::path::Path;
use std::sync::Mutex;

use sherpa_onnx::{
    OfflineMoonshineModelConfig, OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
};

use crate::error::AppError;
use crate::speech::stt::SttEngine;

pub(crate) struct LocalSherpaStt {
    recognizer: Mutex<OfflineRecognizer>,
}

impl LocalSherpaStt {
    pub(crate) fn load(
        model_dir: &Path,
        provider: &str,
        num_threads: i32,
    ) -> Result<Self, AppError> {
        let preprocessor = model_dir.join("preprocess.onnx");
        let encoder = model_dir.join("encode.int8.onnx");
        let uncached_decoder = model_dir.join("uncached_decode.int8.onnx");
        let cached_decoder = model_dir.join("cached_decode.int8.onnx");
        let tokens = model_dir.join("tokens.txt");

        for p in [
            &preprocessor,
            &encoder,
            &uncached_decoder,
            &cached_decoder,
            &tokens,
        ] {
            if !p.exists() {
                return Err(AppError::Config(format!(
                    "Missing STT model file: {}",
                    p.display()
                )));
            }
        }

        let model_config = OfflineModelConfig {
            moonshine: OfflineMoonshineModelConfig {
                preprocessor: Some(preprocessor.to_string_lossy().into_owned()),
                encoder: Some(encoder.to_string_lossy().into_owned()),
                uncached_decoder: Some(uncached_decoder.to_string_lossy().into_owned()),
                cached_decoder: Some(cached_decoder.to_string_lossy().into_owned()),
                merged_decoder: None,
            },
            tokens: Some(tokens.to_string_lossy().into_owned()),
            num_threads,
            debug: false,
            provider: Some(provider.to_string()),
            ..Default::default()
        };

        let config = OfflineRecognizerConfig {
            model_config,
            ..Default::default()
        };

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            AppError::Config("create Moonshine recognizer failed".into())
        })?;

        Ok(Self {
            recognizer: Mutex::new(recognizer),
        })
    }
}

impl SttEngine for LocalSherpaStt {
    fn transcribe(&self, samples: &[f32]) -> Result<String, AppError> {
        let rec = self
            .recognizer
            .lock()
            .map_err(|e| AppError::Internal(format!("STT mutex poisoned: {e}")))?;
        let stream = rec.create_stream();
        stream.accept_waveform(16_000, samples);
        rec.decode(&stream);
        Ok(stream.get_result().map(|r| r.text).unwrap_or_default())
    }
}

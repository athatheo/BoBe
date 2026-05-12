//! Smart-turn v3.2 ONNX inference. Replaces the StubSmartTurn no-op gate.
//!
//! Pipeline:
//!   1. Take the last 8 seconds of audio (128000 samples @16kHz). If
//!      shorter, zero-pad at the START (smart-turn was trained with
//!      pre-utterance silence).
//!   2. Whisper-compatible mel preprocessing: hop=160, fft=400, n_mels=80.
//!      Emit exactly 800 frames so the model input is `[1, 80, 800]` f32.
//!   3. Run the int8 ONNX model via tract; sigmoid the scalar logit; return
//!      the probability that the user finished their thought.
//!
//! Threshold is consumed by the voice handler — this impl just returns the
//! raw probability.

use std::path::Path;
use std::sync::Mutex;

use mel_spec::mel::MelSpectrogram;
use mel_spec::stft::Spectrogram;
use tract_onnx::prelude::*;

use super::turn::SemanticTurn;
use crate::error::AppError;

const SAMPLE_RATE: usize = 16_000;
const WINDOW_SECONDS: usize = 8;
const WINDOW_SAMPLES: usize = SAMPLE_RATE * WINDOW_SECONDS; // 128_000
const FFT_SIZE: usize = 400;
const HOP_SIZE: usize = 160;
const N_MELS: usize = 80;
const N_FRAMES: usize = 800;

type RunnableModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

pub(crate) struct OnnxSmartTurn {
    /// Wrapped in a Mutex so the Tract runnable can be called from concurrent
    /// turns. Inference is single-threaded internally anyway; the lock is
    /// rarely contended because the voice pipeline single-flights turns.
    model: Mutex<RunnableModel>,
}

impl OnnxSmartTurn {
    pub(crate) fn load(model_path: &Path) -> Result<Self, AppError> {
        let model = tract_onnx::onnx()
            .model_for_path(model_path)
            .map_err(|e| AppError::Internal(format!("smart-turn load_path: {e}")))?
            .into_optimized()
            .map_err(|e| AppError::Internal(format!("smart-turn optimize: {e}")))?
            .into_runnable()
            .map_err(|e| AppError::Internal(format!("smart-turn into_runnable: {e}")))?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

impl SemanticTurn for OnnxSmartTurn {
    fn probability_complete(&self, samples: &[f32]) -> Result<f32, AppError> {
        let window = last_window_with_left_pad(samples, WINDOW_SAMPLES);
        let mel = compute_mel_features(&window)?;
        debug_assert_eq!(mel.len(), N_MELS * N_FRAMES);

        let input_tensor: Tensor =
            tract_ndarray::Array3::from_shape_vec((1, N_MELS, N_FRAMES), mel)
                .map_err(|e| AppError::Internal(format!("smart-turn reshape: {e}")))?
                .into_tensor();
        let input: TValue = input_tensor.into();

        let outputs = self
            .model
            .lock()
            .map_err(|_| AppError::Internal("smart-turn model lock poisoned".into()))?
            .run(tvec!(input))
            .map_err(|e| AppError::Internal(format!("smart-turn run: {e}")))?;
        let view = outputs[0]
            .to_array_view::<f32>()
            .map_err(|e| AppError::Internal(format!("smart-turn output view: {e}")))?;
        let raw = view
            .iter()
            .copied()
            .next()
            .ok_or_else(|| AppError::Internal("smart-turn empty output".into()))?;
        // Smart-turn v3 ONNX export emits raw logits; apply sigmoid.
        let prob = 1.0_f32 / (1.0 + (-raw).exp());
        Ok(prob)
    }
}

/// Zero-pad at the START so trailing samples (the user's last word) sit
/// flush with the right edge of the window. If `samples` is longer than
/// `window`, take the tail.
fn last_window_with_left_pad(samples: &[f32], window: usize) -> Vec<f32> {
    if samples.len() >= window {
        return samples[samples.len() - window..].to_vec();
    }
    let mut out = vec![0.0_f32; window];
    let copy_at = window - samples.len();
    out[copy_at..].copy_from_slice(samples);
    out
}

/// Whisper-compatible mel features. Returns a `Vec<f32>` of length
/// `N_MELS * N_FRAMES` in row-major order matching the `[1, 80, 800]`
/// expected by smart-turn v3.
///
/// Frame layout: we run STFT over windows of `HOP_SIZE` samples (with
/// internal overlap-and-save buffering for `FFT_SIZE` context), producing
/// one mel frame per non-`None` result, until we have `N_FRAMES`. The
/// final frame is zero-padded inside `Spectrogram::add` if needed.
fn compute_mel_features(window: &[f32]) -> Result<Vec<f32>, AppError> {
    let mut stft = Spectrogram::new(FFT_SIZE, HOP_SIZE);
    let mut mel = MelSpectrogram::new(FFT_SIZE, SAMPLE_RATE as f64, N_MELS);
    let mut frames = Vec::<f32>::with_capacity(N_MELS * N_FRAMES);
    let mut frame_count = 0_usize;

    for chunk in window.chunks(HOP_SIZE) {
        if frame_count == N_FRAMES {
            break;
        }
        if let Some(fft_frame) = stft.add(chunk) {
            let mel_frame = mel.add(&fft_frame);
            // mel_frame shape: (N_MELS, 1); flatten in column-major to row
            // order matching the [n_mels, n_frames] layout we accumulate
            // into below.
            for row in 0..N_MELS {
                #[allow(clippy::cast_possible_truncation, reason = "f64→f32 expected")]
                frames.push(mel_frame[[row, 0]] as f32);
            }
            frame_count += 1;
        }
    }

    // Pad missing frames with zeros so the tensor is always [N_MELS, N_FRAMES].
    let pad_count = (N_FRAMES - frame_count) * N_MELS;
    frames.resize(frames.len() + pad_count, 0.0);

    // We accumulated as [frame, mel] (interleaved). Reshape to [mel, frame]
    // row-major because the model expects channels-then-time.
    let mut transposed = vec![0.0_f32; N_MELS * N_FRAMES];
    for frame_idx in 0..N_FRAMES {
        for mel_idx in 0..N_MELS {
            transposed[mel_idx * N_FRAMES + frame_idx] = frames[frame_idx * N_MELS + mel_idx];
        }
    }
    Ok(transposed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_left_zero_when_shorter_than_window() {
        let samples = vec![1.0_f32; 100];
        let win = last_window_with_left_pad(&samples, 200);
        assert_eq!(win.len(), 200);
        assert!(win[..100].iter().all(|&x| x == 0.0));
        assert!(win[100..].iter().all(|&x| (x - 1.0).abs() < 1e-6));
    }

    #[test]
    fn take_tail_when_longer_than_window() {
        let samples: Vec<f32> = (0..300).map(|i| i as f32).collect();
        let win = last_window_with_left_pad(&samples, 100);
        assert_eq!(win.len(), 100);
        assert_eq!(win[0], 200.0);
        assert_eq!(win[99], 299.0);
    }

    #[test]
    fn mel_features_have_expected_length() {
        // Real audio not required — silent input still produces 80*800 mel bins.
        let samples = vec![0.0_f32; WINDOW_SAMPLES];
        let mel = compute_mel_features(&samples).unwrap();
        assert_eq!(mel.len(), N_MELS * N_FRAMES);
    }
}

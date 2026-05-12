//! Shared Opus encoding helpers used by both the voice WS handler
//! (kokoro task + filler watchdog) and the BobeHooks PreToolUse path
//! (per-tool cached fillers). Kept here so hooks don't need to depend
//! on the api::handlers::voice module.

use opus::{Application, Channels, Encoder as OpusEncoder};
use tracing::warn;

/// 24kbps VoIP profile, matches the WS protocol commitment in voice.rs.
const TTS_OPUS_BITRATE_BPS: i32 = 24_000;

/// Build a fresh Opus encoder for 20ms VoIP-profile frames at the given
/// sample rate. Returns `None` and warns on failure; callers fall through
/// silently rather than crash the turn.
pub(crate) fn make_opus_encoder(sample_rate: u32) -> Option<OpusEncoder> {
    let mut encoder = match OpusEncoder::new(sample_rate, Channels::Mono, Application::Voip) {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "voice.opus_encoder_failed");
            return None;
        }
    };
    if let Err(e) = encoder.set_bitrate(opus::Bitrate::Bits(TTS_OPUS_BITRATE_BPS)) {
        warn!(error = %e, "voice.opus_bitrate_failed");
    }
    Some(encoder)
}

/// Encode PCM samples into 20ms Opus frames using an existing encoder.
/// Reusing the encoder across sentences preserves internal entropy-coder
/// state for marginally better compression than per-call construction.
pub(crate) fn encode_pcm_with(
    encoder: &mut OpusEncoder,
    pcm: &[f32],
    sample_rate: u32,
) -> Vec<Vec<u8>> {
    let frame_samples = (sample_rate as usize) / 50;
    let mut frames = Vec::new();
    for chunk in pcm.chunks(frame_samples) {
        let mut input = chunk.to_vec();
        if input.len() < frame_samples {
            input.resize(frame_samples, 0.0);
        }
        let pcm_i16: Vec<i16> = input
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32_767.0) as i16)
            .collect();
        let mut out = vec![0_u8; 1500];
        match encoder.encode(&pcm_i16, &mut out) {
            Ok(n) => {
                out.truncate(n);
                frames.push(out);
            }
            Err(e) => {
                warn!(error = %e, "voice.opus_encode_failed");
                break;
            }
        }
    }
    frames
}

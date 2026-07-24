use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const BODY_SUBPROTOCOL_V1: &str = "bobe.body.v1";
pub(crate) const ADAPTER_SUBPROTOCOL_V1: &str = "bobe.speech-adapter.v1";
pub(crate) const BODY_MDNS_AUTH: &str = "mtls-pinned";
pub(crate) const PROTOCOL_MAJOR: u8 = 1;
pub(crate) const PROTOCOL_MINOR: u8 = 1;
pub(crate) const MAX_CONTROL_BYTES: usize = 4_096;
pub(crate) const MEDIA_HEADER_LEN: usize = 20;
pub(crate) const MIC_PCM_BYTES: usize = 640;
pub(crate) const SPEAKER_PCM_BYTES: usize = 960;
pub(crate) const ADAPTER_MEDIA_HEADER_LEN: usize = 40;
pub(crate) const CAPTURE_CODEC: &str = "pcm_s16le";
pub(crate) const CAPTURE_SAMPLE_RATE_HZ: u32 = 16_000;
pub(crate) const CAPTURE_CHANNELS: u8 = 1;
pub(crate) const AUDIO_FRAME_DURATION_MS: u16 = 20;
pub(crate) const MAX_PLAYBACK_CREDIT_MS: u32 = 120;
pub(crate) const MAX_LEASE_LIFETIME_MS: u32 = 600_000;
pub(crate) const TTS_CODEC: &str = "opus";
pub(crate) const TTS_SAMPLE_RATE_HZ: u32 = 24_000;
pub(crate) const TTS_CHANNELS: u8 = 1;
pub(crate) const SPEAKER_CODEC: &str = "pcm_s16le";
pub(crate) const SPEAKER_SAMPLE_RATE_HZ: u32 = 24_000;
pub(crate) const SPEAKER_CHANNELS: u8 = 1;

#[derive(Debug, Deserialize)]
pub(crate) struct BodyHello {
    pub(crate) protocol_major: u8,
    #[serde(default)]
    pub(crate) protocol_minor: u8,
    pub(crate) device_id: String,
    pub(crate) boot_id: String,
    pub(crate) firmware: String,
    pub(crate) hardware: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BodyWelcome<'a> {
    pub(crate) r#type: &'static str,
    pub(crate) protocol_major: u8,
    pub(crate) protocol_minor: u8,
    pub(crate) session_id: &'a str,
    pub(crate) connection_generation: u64,
    pub(crate) heartbeat_interval_ms: u32,
    pub(crate) controller_epoch: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CaptureRequest {
    pub(crate) request_id: String,
    pub(crate) stream_id: u32,
    pub(crate) trigger: String,
    pub(crate) codec: String,
    pub(crate) sample_rate_hz: u32,
    pub(crate) channels: u8,
    pub(crate) frame_duration_ms: u16,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CaptureCancel {
    pub(crate) request_id: String,
    pub(crate) stream_id: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct CaptureDecision<'a> {
    pub(crate) r#type: &'static str,
    pub(crate) protocol_major: u8,
    pub(crate) request_id: &'a str,
    pub(crate) stream_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lease_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) turn_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lease_ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RouteDescriptor {
    pub(crate) device_id: String,
    pub(crate) connection_generation: u64,
    pub(crate) body_session_id: Uuid,
    pub(crate) lease_id: Uuid,
    pub(crate) turn_id: String,
    pub(crate) capture_stream_id: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RoutedStreamControl {
    pub(crate) lease_id: Uuid,
    pub(crate) turn_id: String,
    pub(crate) stream_id: u32,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlaybackCredit {
    pub(crate) lease_id: Uuid,
    pub(crate) turn_id: String,
    pub(crate) stream_id: u32,
    pub(crate) credit_ms: u32,
    #[serde(default)]
    pub(crate) played_samples: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdapterWelcome {
    pub(crate) r#type: &'static str,
    pub(crate) protocol_major: u8,
    pub(crate) protocol_minor: u8,
    pub(crate) adapter_generation: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct AdapterAudioFormat {
    pub(crate) codec: String,
    pub(crate) sample_rate_hz: u32,
    pub(crate) channels: u8,
    pub(crate) frame_duration_ms: u16,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(crate) enum AdapterOutbound {
    #[serde(rename = "capture.open")]
    CaptureOpen {
        route: RouteDescriptor,
        codec: &'static str,
        sample_rate_hz: u32,
        channels: u8,
        frame_duration_ms: u16,
    },
    #[serde(rename = "capture.close")]
    CaptureClose { route: RouteDescriptor },
    #[serde(rename = "turn.cancel")]
    TurnCancel {
        route: RouteDescriptor,
        reason: &'static str,
    },
    #[serde(rename = "turn.complete")]
    TurnComplete {
        route: RouteDescriptor,
        reason: &'static str,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AdapterInbound {
    #[serde(rename = "adapter.hello")]
    Hello {
        protocol_major: u8,
        #[serde(default)]
        protocol_minor: u8,
        capture_format: AdapterAudioFormat,
        tts_format: AdapterAudioFormat,
        speaker_format: AdapterAudioFormat,
    },
    #[serde(rename = "transcript.partial")]
    TranscriptPartial {
        connection_generation: u64,
        lease_id: Uuid,
        text: String,
    },
    #[serde(rename = "transcript.final")]
    TranscriptFinal {
        connection_generation: u64,
        lease_id: Uuid,
        text: String,
    },
    #[serde(rename = "transcript.empty")]
    TranscriptEmpty {
        connection_generation: u64,
        lease_id: Uuid,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MediaHeader {
    pub(crate) kind: u8,
    pub(crate) flags: u16,
    pub(crate) stream_id: u32,
    pub(crate) sequence: u32,
    pub(crate) monotonic_us: u64,
}

impl MediaHeader {
    pub(crate) fn parse(data: &[u8]) -> Option<Self> {
        let header = data.get(..MEDIA_HEADER_LEN)?;
        if header[0] != PROTOCOL_MAJOR {
            return None;
        }
        let kind = header[1];
        let flags = u16::from_be_bytes([header[2], header[3]]);
        let stream_id = u32::from_be_bytes(header[4..8].try_into().ok()?);
        let sequence = u32::from_be_bytes(header[8..12].try_into().ok()?);
        let monotonic_us = u64::from_be_bytes(header[12..20].try_into().ok()?);
        if !matches!(kind, 0x01..=0x04) || flags & !0x0003 != 0 || stream_id == 0 {
            return None;
        }
        Some(Self {
            kind,
            flags,
            stream_id,
            sequence,
            monotonic_us,
        })
    }

    pub(crate) fn validate_microphone_frame(data: &[u8]) -> Option<Self> {
        let header = Self::parse(data)?;
        (header.kind == 0x01 && data.len() == MEDIA_HEADER_LEN + MIC_PCM_BYTES).then_some(header)
    }

    pub(crate) fn encode_speaker_frame(
        stream_id: u32,
        sequence: u32,
        monotonic_us: u64,
        pcm: &[u8],
    ) -> Option<Vec<u8>> {
        if stream_id == 0 || pcm.len() != SPEAKER_PCM_BYTES {
            return None;
        }
        let mut frame = Vec::with_capacity(MEDIA_HEADER_LEN + pcm.len());
        frame.push(PROTOCOL_MAJOR);
        frame.push(0x02);
        frame.extend_from_slice(&0_u16.to_be_bytes());
        frame.extend_from_slice(&stream_id.to_be_bytes());
        frame.extend_from_slice(&sequence.to_be_bytes());
        frame.extend_from_slice(&monotonic_us.to_be_bytes());
        frame.extend_from_slice(pcm);
        Some(frame)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum AdapterMediaKind {
    MicrophonePcm = 1,
    TtsOpus = 2,
    SpeakerPcm = 3,
}

impl TryFrom<u8> for AdapterMediaKind {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::MicrophonePcm),
            2 => Ok(Self::TtsOpus),
            3 => Ok(Self::SpeakerPcm),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdapterMediaHeader {
    pub(crate) kind: AdapterMediaKind,
    pub(crate) connection_generation: u64,
    pub(crate) lease_id: Uuid,
    pub(crate) stream_id: u32,
    pub(crate) sequence: u32,
}

impl AdapterMediaHeader {
    pub(crate) fn encode(self, payload: &[u8]) -> Vec<u8> {
        let mut output = Vec::with_capacity(ADAPTER_MEDIA_HEADER_LEN + payload.len());
        output.push(PROTOCOL_MAJOR);
        output.push(self.kind as u8);
        output.extend_from_slice(&0_u16.to_be_bytes());
        output.extend_from_slice(&self.connection_generation.to_be_bytes());
        output.extend_from_slice(self.lease_id.as_bytes());
        output.extend_from_slice(&self.stream_id.to_be_bytes());
        output.extend_from_slice(&self.sequence.to_be_bytes());
        output.extend_from_slice(&0_u32.to_be_bytes());
        output.extend_from_slice(payload);
        output
    }

    pub(crate) fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        let header = data.get(..ADAPTER_MEDIA_HEADER_LEN)?;
        if header[0] != PROTOCOL_MAJOR || header[2..4] != [0, 0] || header[36..40] != [0; 4] {
            return None;
        }
        let kind = AdapterMediaKind::try_from(header[1]).ok()?;
        let connection_generation = u64::from_be_bytes(header[4..12].try_into().ok()?);
        let lease_id = Uuid::from_bytes(header[12..28].try_into().ok()?);
        let stream_id = u32::from_be_bytes(header[28..32].try_into().ok()?);
        let sequence = u32::from_be_bytes(header[32..36].try_into().ok()?);
        if connection_generation == 0 || stream_id == 0 {
            return None;
        }
        Some((
            Self {
                kind,
                connection_generation,
                lease_id,
                stream_id,
                sequence,
            },
            &data[ADAPTER_MEDIA_HEADER_LEN..],
        ))
    }
}

pub(crate) fn control_type(value: &serde_json::Value) -> Option<&str> {
    value.get("type")?.as_str()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn adapter_media_round_trip_preserves_route() {
        let lease_id = Uuid::new_v4();
        let header = AdapterMediaHeader {
            kind: AdapterMediaKind::SpeakerPcm,
            connection_generation: 7,
            lease_id,
            stream_id: 42,
            sequence: 3,
        };
        let encoded = header.encode(&[1, 2, 3]);
        let (decoded, payload) = AdapterMediaHeader::parse(&encoded).expect("valid adapter frame");
        assert_eq!(decoded, header);
        assert_eq!(payload, [1, 2, 3]);
    }

    #[test]
    fn body_media_rejects_wrong_size_and_zero_stream() {
        let mut frame = vec![0_u8; MEDIA_HEADER_LEN + MIC_PCM_BYTES];
        frame[0] = PROTOCOL_MAJOR;
        frame[1] = 0x01;
        assert!(MediaHeader::validate_microphone_frame(&frame).is_none());
        frame[7] = 1;
        assert!(MediaHeader::validate_microphone_frame(&frame).is_some());
        frame.pop();
        assert!(MediaHeader::validate_microphone_frame(&frame).is_none());
    }
}

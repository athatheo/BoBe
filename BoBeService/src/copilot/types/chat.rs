//! Streaming chat protocol — prompt input, attachments, delta events.
//! Used by the Chat worker class (the only one with a streaming
//! response loop). Voice turns flag `voice_mode=true` so the SDK
//! routes the prompt through `DeliveryMode::Immediate` (atomic
//! interrupt of any in-flight LLM generation).

#[derive(Debug, Clone, Default)]
pub(crate) struct ChatPrompt {
    pub(crate) text: String,
    pub(crate) attachments: Vec<ChatAttachment>,
    /// Voice turns ride `DeliveryMode::Immediate` so a new transcript atomically
    /// interrupts any in-flight LLM generation server-side. Default `false` keeps
    /// text chat on the SDK's default `Enqueue` behavior.
    pub(crate) voice_mode: bool,
}

impl ChatPrompt {
    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            attachments: Vec::new(),
            voice_mode: false,
        }
    }

    pub(crate) fn voice(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            attachments: Vec::new(),
            voice_mode: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ChatAttachment {
    /// Lifted to `Attachment::Blob` (base64 inline, no disk I/O).
    ImageBytes {
        bytes: Vec<u8>,
        mime_type: &'static str,
    },
}

impl From<ChatAttachment> for github_copilot_sdk::types::Attachment {
    fn from(att: ChatAttachment) -> Self {
        use base64::Engine;
        let base64 = base64::engine::general_purpose::STANDARD;
        let ChatAttachment::ImageBytes { bytes, mime_type } = att;
        Self::Blob {
            data: base64.encode(&bytes),
            mime_type: mime_type.to_string(),
            display_name: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ChatDelta {
    MessageDelta(String),
    /// Authoritative final body; deltas may have been re-streamed out-of-order.
    MessageComplete {
        content: String,
        output_tokens: Option<u64>,
    },
    ToolStart {
        id: String,
        name: String,
    },
    /// `success` reflects exit, not model satisfaction.
    ToolComplete {
        id: String,
        name: String,
        success: bool,
    },
    Error(String),
    Done,
}

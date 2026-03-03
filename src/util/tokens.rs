use std::sync::LazyLock;

use tiktoken_rs::CoreBPE;

/// o200k_base encoding — used by gpt-4o, gpt-5, and newer OpenAI models.
///
/// The BPE data is compiled into the binary by tiktoken-rs; `o200k_base()` can
/// only fail on a corrupt binary, making this logically impossible at runtime.
static O200K: LazyLock<CoreBPE> =
    LazyLock::new(|| tiktoken_rs::o200k_base().expect("o200k_base data compiled into binary"));

use crate::llm::types::AiMessage;

/// Count tokens in text using the o200k_base encoding (gpt-5 family).
///
/// For non-OpenAI backends (Ollama, llama.cpp), this is an approximation
/// but still more accurate than byte-length heuristics.
pub fn count_tokens(text: &str) -> usize {
    O200K.encode_with_special_tokens(text).len()
}

/// Per-message overhead: role label, delimiters, separators.
const MESSAGE_OVERHEAD_TOKENS: usize = 4;

/// Minimum response budget — never clamp below this.
const MIN_RESPONSE_TOKENS: u32 = 100;

/// Count tokens across a message array.
///
/// Adds ~4 tokens per message for role/delimiter overhead (OpenAI convention).
pub fn count_message_tokens(messages: &[AiMessage]) -> usize {
    messages
        .iter()
        .map(|msg| {
            let text_tokens = count_tokens(msg.content.text_or_empty());
            text_tokens + MESSAGE_OVERHEAD_TOKENS
        })
        .sum()
}

/// Clamp `max_tokens` so prompt + response fits within `context_window`.
///
/// Returns `min(requested, context_window - prompt_tokens)`, floored at
/// [`MIN_RESPONSE_TOKENS`]. If the prompt already exceeds the context window,
/// returns the floor — the LLM will likely error, but at least we don't
/// request a zero-length response.
pub fn clamp_max_tokens(context_window: u32, prompt_tokens: usize, requested: u32) -> u32 {
    let available = (context_window as usize).saturating_sub(prompt_tokens);
    // Safe: available ≤ context_window ≤ u32::MAX
    let clamped = (available as u32).min(requested);
    clamped.max(MIN_RESPONSE_TOKENS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_message_tokens_empty() {
        assert_eq!(count_message_tokens(&[]), 0);
    }

    #[test]
    fn count_message_tokens_single() {
        let msgs = vec![AiMessage::user("Hello, world!")];
        let count = count_message_tokens(&msgs);
        // "Hello, world!" is ~4 tokens + 4 overhead = ~8
        assert!(count > 0);
        assert!(count < 20);
    }

    #[test]
    fn count_message_tokens_multiple() {
        let msgs = vec![
            AiMessage::system("You are a helpful assistant."),
            AiMessage::user("What is 2+2?"),
        ];
        let count = count_message_tokens(&msgs);
        // Two messages, each with content + overhead
        assert!(count > 8);
    }

    #[test]
    fn clamp_max_tokens_no_clamping_needed() {
        // 128K window, 1K prompt, requesting 500 → 500
        assert_eq!(clamp_max_tokens(128_000, 1_000, 500), 500);
    }

    #[test]
    fn clamp_max_tokens_clamped() {
        // 4K window, 3500 prompt, requesting 1000 → 596 (clamped to available)
        assert_eq!(clamp_max_tokens(4_096, 3_500, 1_000), 596);
    }

    #[test]
    fn clamp_max_tokens_floor() {
        // 4K window, 4090 prompt, requesting 1000 → floor at 100
        assert_eq!(clamp_max_tokens(4_096, 4_090, 1_000), MIN_RESPONSE_TOKENS);
    }

    #[test]
    fn clamp_max_tokens_prompt_exceeds_window() {
        // Prompt larger than window → floor at 100
        assert_eq!(clamp_max_tokens(4_096, 5_000, 1_000), MIN_RESPONSE_TOKENS);
    }

    #[test]
    fn clamp_max_tokens_exact_fit() {
        // 1000 window, 500 prompt, requesting 500 → 500
        assert_eq!(clamp_max_tokens(1_000, 500, 500), 500);
    }
}

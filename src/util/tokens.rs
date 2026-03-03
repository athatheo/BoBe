use std::sync::LazyLock;

use tiktoken_rs::CoreBPE;

/// o200k_base encoding — used by gpt-4o, gpt-4o-mini, and newer OpenAI models.
///
/// The BPE data is compiled into the binary by tiktoken-rs; `o200k_base()` can
/// only fail on a corrupt binary, making this logically impossible at runtime.
static O200K: LazyLock<CoreBPE> =
    LazyLock::new(|| tiktoken_rs::o200k_base().expect("o200k_base data compiled into binary"));

/// Count tokens in text using the o200k_base encoding (gpt-4o family).
///
/// For non-OpenAI backends (Ollama, llama.cpp), this is an approximation
/// but still more accurate than byte-length heuristics.
pub fn count_tokens(text: &str) -> usize {
    O200K.encode_with_special_tokens(text).len()
}

#!/usr/bin/env bash
#
# Fetch the test corpora used by `cargo test --test voice_corpora -- --ignored`.
# Idempotent — re-run safely; existing files are kept.
#
#   LibriSpeech test-clean  ~ 350MB  (CC-BY 4.0)
#   smart-turn-data-v3-test ~  20MB  (HuggingFace)
#
# Total disk: ~370MB at ~/.cache/bobe-tests/.

set -euo pipefail

ROOT="${BOBE_TEST_CORPORA_ROOT:-$HOME/.cache/bobe-tests}"
mkdir -p "$ROOT"

echo "Fetching test corpora into $ROOT…"

# --- LibriSpeech test-clean
LS_DIR="$ROOT/librispeech"
LS_TARBALL="$ROOT/librispeech-test-clean.tar.gz"
if [[ ! -d "$LS_DIR" ]]; then
    if [[ ! -f "$LS_TARBALL" ]]; then
        echo "  ↳ downloading LibriSpeech test-clean…"
        curl -fL --output "$LS_TARBALL" \
            "https://www.openslr.org/resources/12/test-clean.tar.gz"
    fi
    echo "  ↳ extracting LibriSpeech test-clean…"
    tar -xzf "$LS_TARBALL" -C "$ROOT"
    # The tarball extracts as LibriSpeech/test-clean — flatten so tests can
    # treat ~/.cache/bobe-tests/librispeech as the corpus root.
    if [[ -d "$ROOT/LibriSpeech/test-clean" ]]; then
        mv "$ROOT/LibriSpeech/test-clean" "$LS_DIR"
        rmdir "$ROOT/LibriSpeech" 2>/dev/null || true
    fi
else
    echo "  ↳ LibriSpeech already present"
fi

# --- smart-turn-data-v3-test
ST_DIR="$ROOT/smart-turn"
if [[ ! -d "$ST_DIR" ]]; then
    mkdir -p "$ST_DIR"
    echo "  ↳ downloading smart-turn-data-v3-test…"
    # The HF dataset lives under datasets/pipecat-ai/smart-turn-data-v3.
    # We pull just the test split via the HF datasets tar API for now.
    # When `huggingface-cli` is installed, prefer that for resumable downloads.
    if command -v huggingface-cli >/dev/null 2>&1; then
        huggingface-cli download \
            pipecat-ai/smart-turn-data-v3 \
            --repo-type dataset \
            --include 'test/*' \
            --local-dir "$ST_DIR"
    else
        echo "  ↳ huggingface-cli not found; install it for smart-turn corpus download:"
        echo "      pipx install huggingface_hub[cli]"
        echo "    skipping for now — smart-turn tier-3 test will be skipped."
    fi
else
    echo "  ↳ smart-turn already present"
fi

echo "Done. Run tier-3 tests with:"
echo "  cargo test --manifest-path BoBeService/Cargo.toml --test voice_corpora -- --ignored --nocapture"

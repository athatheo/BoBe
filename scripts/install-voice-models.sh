#!/usr/bin/env bash
#
# Voice model installer for BoBe.
#
# Downloads the three ONNX/tarball assets the /voice/stream pipeline needs
# and lays them out at the paths bootstrap::load_voice_engines() expects:
#
#   ~/.bobe/models/sherpa-onnx-moonshine-base-en-int8/    (STT, ~150 MB)
#   ~/.bobe/models/kokoro-multi-lang-v1_0/                (TTS, ~340 MB)
#   ~/.bobe/models/silero-vad/silero_vad.onnx             (VAD, ~2 MB)
#
# Idempotent: skips any model that's already present. Re-run to fetch
# missing pieces only. Total fresh install pulls ~490 MB.
#
# Manual replacement for the full M4.5.0a model_manager + welcome-wizard
# install flow — minimum viable "I want to try voice".

set -euo pipefail

MODELS_DIR="${BOBE_MODELS_DIR:-${HOME}/.bobe/models}"
SHERPA_RELEASE_BASE="https://github.com/k2-fsa/sherpa-onnx/releases/download"
SILERO_RAW_BASE="https://github.com/snakers4/silero-vad/raw"
SMART_TURN_HF_BASE="https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main"

cyan() { printf "\033[36m%s\033[0m\n" "$*"; }
green() { printf "\033[32m%s\033[0m\n" "$*"; }
red() { printf "\033[31m%s\033[0m\n" "$*" >&2; }
gray() { printf "\033[90m%s\033[0m\n" "$*"; }

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        red "missing required command: $1"
        exit 1
    fi
}

ensure_extracted_tarball() {
    local archive_url="$1"
    local target_dir="$2"
    local dir_basename
    dir_basename="$(basename "$target_dir")"

    if [[ -d "$target_dir" ]]; then
        gray "  ✓ $dir_basename already installed"
        return 0
    fi

    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    local archive="$tmp/$(basename "$archive_url")"

    cyan "  Downloading $dir_basename..."
    curl -L --fail --progress-bar -o "$archive" "$archive_url"

    cyan "  Extracting..."
    tar -xjf "$archive" -C "$tmp"

    # The tarball extracts to a sibling dir of the same basename as the archive
    # (minus the .tar.bz2). Move it into the models root with the canonical name.
    local extracted
    extracted="$(find "$tmp" -maxdepth 1 -type d -name "${dir_basename}*" -print -quit)"
    if [[ -z "$extracted" || ! -d "$extracted" ]]; then
        red "tarball $archive_url didn't contain a $dir_basename directory"
        return 1
    fi
    mkdir -p "$(dirname "$target_dir")"
    mv "$extracted" "$target_dir"
    green "  ✓ $dir_basename installed"
    trap - EXIT
    rm -rf "$tmp"
}

install_streaming_zipformer_en() {
    local target_dir="$MODELS_DIR/sherpa-onnx-streaming-zipformer-en"
    if [[ -d "$target_dir" ]]; then
        gray "  ✓ sherpa-onnx-streaming-zipformer-en already installed"
        return 0
    fi
    local archive_url="$SHERPA_RELEASE_BASE/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2"
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    local archive="$tmp/zipformer.tar.bz2"
    cyan "  Downloading sherpa-onnx-streaming-zipformer-en..."
    curl -L --fail --progress-bar -o "$archive" "$archive_url"
    cyan "  Extracting..."
    tar -xjf "$archive" -C "$tmp"
    local extracted
    extracted="$(find "$tmp" -maxdepth 1 -type d -name "sherpa-onnx-streaming-zipformer-en*" -print -quit)"
    if [[ -z "$extracted" || ! -d "$extracted" ]]; then
        red "tarball did not contain a sherpa-onnx-streaming-zipformer-en directory"
        return 1
    fi
    # Rename the canonical encoder/decoder/joiner files to drop the
    # epoch-NN-avg-N suffixes; the daemon loads encoder.onnx etc.
    local enc dec join
    enc="$(find "$extracted" -maxdepth 1 -type f -name 'encoder*.onnx' | head -1)"
    dec="$(find "$extracted" -maxdepth 1 -type f -name 'decoder*.onnx' | head -1)"
    join="$(find "$extracted" -maxdepth 1 -type f -name 'joiner*.onnx' | head -1)"
    if [[ -z "$enc" || -z "$dec" || -z "$join" ]]; then
        red "tarball missing encoder/decoder/joiner ONNX"
        return 1
    fi
    mv "$enc" "$extracted/encoder.onnx"
    mv "$dec" "$extracted/decoder.onnx"
    mv "$join" "$extracted/joiner.onnx"
    mkdir -p "$(dirname "$target_dir")"
    mv "$extracted" "$target_dir"
    green "  ✓ sherpa-onnx-streaming-zipformer-en installed"
    trap - EXIT
    rm -rf "$tmp"
}

ensure_single_file() {
    local file_url="$1"
    local target_file="$2"
    local file_basename
    file_basename="$(basename "$target_file")"

    if [[ -f "$target_file" ]]; then
        gray "  ✓ $file_basename already installed"
        return 0
    fi

    cyan "  Downloading $file_basename..."
    mkdir -p "$(dirname "$target_file")"
    curl -L --fail --progress-bar -o "$target_file.part" "$file_url"
    mv "$target_file.part" "$target_file"
    green "  ✓ $file_basename installed"
}

main() {
    require_cmd curl
    require_cmd tar

    cyan "Installing voice models into $MODELS_DIR"
    mkdir -p "$MODELS_DIR"

    # STT — sherpa-onnx streaming Zipformer English. Emits partials live as
    # frames arrive, which Moonshine couldn't do; unlocks MinWords barge-in
    # + cancel-phrase detection. Tarball contents are renamed to canonical
    # names the daemon's loader expects (encoder.onnx etc.) so version
    # drift on the upstream artifact doesn't break us.
    install_streaming_zipformer_en


    # TTS — sherpa-onnx Kokoro v1.0 multilingual
    ensure_extracted_tarball \
        "$SHERPA_RELEASE_BASE/tts-models/kokoro-multi-lang-v1_0.tar.bz2" \
        "$MODELS_DIR/kokoro-multi-lang-v1_0"

    # Acoustic VAD — Silero v6.2.1 ONNX
    ensure_single_file \
        "$SILERO_RAW_BASE/v6.2.1/src/silero_vad/data/silero_vad.onnx" \
        "$MODELS_DIR/silero-vad/silero_vad.onnx"

    # Semantic VAD — Pipecat smart-turn v3 (int8 ONNX, ~8MB). The daemon
    # looks for smart-turn-v3.2.int8.onnx so the v3 download is symlinked
    # under that name. Daily.co's v3.2 paper-tagged release uses the same
    # tensor layout; if a v3.2-only ONNX appears at Pipecat-AI on HF, swap
    # the URL here.
    ensure_single_file \
        "$SMART_TURN_HF_BASE/smart_turn_v3.0.int8.onnx" \
        "$MODELS_DIR/smart-turn-v3.2.int8.onnx"

    echo
    green "Voice models installed. Restart the BoBe daemon to pick them up."
}

main "$@"

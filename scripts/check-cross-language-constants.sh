#!/usr/bin/env bash
# Assert Rust <-> Swift cross-language constants match.
#
# Rust source: BoBeService/src/constants.rs
# Swift source: BoBeMacUI/BoBe/App/Constants.swift
#
# If a value drifts between sides (port, engine kind, MCP status, Ollama URL),
# the Swift client and Rust daemon will silently disagree at runtime. This
# script greps both sources and asserts each pair matches.
#
# Run from `just check`; exit 1 on drift.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST="$ROOT/BoBeService/src/constants.rs"
SWIFT_CONST="$ROOT/BoBeMacUI/BoBe/App/Constants.swift"

fail=0

expect_match() {
    local name="$1" rust_value="$2" swift_value="$3"
    if [[ "$rust_value" != "$swift_value" ]]; then
        echo "drift: $name — Rust='$rust_value' Swift='$swift_value'" >&2
        fail=1
    fi
}

# DEFAULT_DAEMON_PORT
rust_port=$(grep -E 'DEFAULT_DAEMON_PORT: u16 =' "$RUST" | sed -E 's/.*= ([0-9]+);.*/\1/')
swift_port=$(grep -E 'static let port = ' "$SWIFT_CONST" | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "daemon port" "$rust_port" "$swift_port"

# DEFAULT_OLLAMA_BASE_URL
rust_ollama_base=$(grep -E 'DEFAULT_OLLAMA_BASE_URL:' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_ollama_base=$(grep -E 'static let baseURL = "http://127' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "ollama base URL" "$rust_ollama_base" "$swift_ollama_base"

# DEFAULT_OLLAMA_V1_URL
rust_ollama_v1=$(grep -E 'DEFAULT_OLLAMA_V1_URL:' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_ollama_v1=$(grep -E 'static let v1URL = ' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "ollama v1 URL" "$rust_ollama_v1" "$swift_ollama_v1"

# engine_kind::*
extract_rust_engine() { grep -E "pub\(crate\) const $1: &str =" "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/'; }
extract_swift_engine() { grep -E "static let $1 = " "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/'; }
expect_match "engine_kind::COPILOT_CLOUD" "$(extract_rust_engine COPILOT_CLOUD)" "$(extract_swift_engine copilotCloud)"
expect_match "engine_kind::LOCAL"         "$(extract_rust_engine LOCAL)"         "$(extract_swift_engine local)"

# mcp_status::*
expect_match "mcp_status::CONNECTED"      "$(extract_rust_engine CONNECTED)"      "$(extract_swift_engine connected)"
expect_match "mcp_status::FAILED"         "$(extract_rust_engine FAILED)"         "$(extract_swift_engine failed)"
expect_match "mcp_status::NEEDS_AUTH"     "$(extract_rust_engine NEEDS_AUTH)"     "$(extract_swift_engine needsAuth)"
expect_match "mcp_status::PENDING"        "$(extract_rust_engine PENDING)"        "$(extract_swift_engine pending)"
expect_match "mcp_status::DISABLED"       "$(extract_rust_engine DISABLED)"       "$(extract_swift_engine disabled)"
expect_match "mcp_status::NOT_CONFIGURED" "$(extract_rust_engine NOT_CONFIGURED)" "$(extract_swift_engine notConfigured)"
expect_match "mcp_status::UNKNOWN"        "$(extract_rust_engine UNKNOWN)"        "$(extract_swift_engine unknown)"

# voice_wire::*
rust_sample=$(grep -E 'TTS_OUTPUT_SAMPLE_RATE: u32 =' "$RUST" | sed -E 's/.*= ([0-9_]+);.*/\1/' | tr -d '_')
swift_sample=$(grep -E 'static let ttsOutputSampleRate = ' "$SWIFT_CONST" | sed -E 's/.*= ([0-9_]+).*/\1/' | tr -d '_')
expect_match "voice TTS sample rate" "$rust_sample" "$swift_sample"

rust_kokoro=$(grep -E 'KOKORO_MODEL_DIR: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_kokoro=$(grep -E 'static let kokoroModelDir = ' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "Kokoro model dir name" "$rust_kokoro" "$swift_kokoro"

rust_persona=$(grep -E 'DEFAULT_PERSONA: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_persona=$(grep -E 'static let defaultPersona = ' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "voice default persona" "$rust_persona" "$swift_persona"

rust_kind_tts=$(grep -E 'MODEL_KIND_TTS: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_kind_tts=$(grep -E 'static let modelKindTts = ' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "voice model kind TTS" "$rust_kind_tts" "$swift_kind_tts"

# tool_call_status::*
rust_tc_start=$(grep -E 'pub\(crate\) const START: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_tc_start=$(grep -E 'static let start = "[^"]+"' "$SWIFT_CONST" | head -n 1 | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "tool_call_status::START" "$rust_tc_start" "$swift_tc_start"

rust_tc_complete=$(grep -E 'pub\(crate\) const COMPLETE: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_tc_complete=$(grep -E 'static let complete = "[^"]+"' "$SWIFT_CONST" | head -n 1 | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "tool_call_status::COMPLETE" "$rust_tc_complete" "$swift_tc_complete"

# BOBE_DATA_DIR_NAME
rust_data_dir=$(grep -E 'BOBE_DATA_DIR_NAME: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_data_dir=$(grep -E 'static let dataDirName = ' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "BOBE_DATA_DIR_NAME" "$rust_data_dir" "$swift_data_dir"

if [[ $fail -eq 0 ]]; then
    echo "cross-language constants ok"
fi
exit $fail

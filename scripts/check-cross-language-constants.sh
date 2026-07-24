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
SMOKE_SCRIPT="$ROOT/scripts/smoke-copilot-tools.sh"
PROFILE_SCRIPT="$ROOT/scripts/profile-daemon-startup.sh"

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
swift_port=$(grep -E 'static let defaultPort = ' "$SWIFT_CONST" | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "daemon port" "$rust_port" "$swift_port"

# Privacy purge client/probe timeout. The daemon owns shorter internal and
# route deadlines; every caller must allow the canonical response margin.
rust_privacy_timeout=$(grep -E 'CLIENT_TIMEOUT_SECS: u64 =' "$RUST" | sed -E 's/.*= ([0-9]+);.*/\1/')
swift_privacy_timeout=$(grep -E 'purgeRequestTimeoutSeconds: TimeInterval =' "$SWIFT_CONST" | sed -E 's/.*= ([0-9]+).*/\1/')
smoke_privacy_timeout=$(grep -E '^PRIVACY_TIMEOUT_SECS=' "$SMOKE_SCRIPT" | sed -E 's/.*=([0-9]+)/\1/')
profile_privacy_timeout=$(grep -E '^PRIVACY_TIMEOUT_SECS=' "$PROFILE_SCRIPT" | sed -E 's/.*=([0-9]+)/\1/')
expect_match "privacy purge Swift timeout" "$rust_privacy_timeout" "$swift_privacy_timeout"
expect_match "privacy purge smoke timeout" "$rust_privacy_timeout" "$smoke_privacy_timeout"
expect_match "privacy purge profile timeout" "$rust_privacy_timeout" "$profile_privacy_timeout"

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

rust_subprotocol=$(grep -E 'SUBPROTOCOL_V1: &str =' "$RUST" | sed -E 's/.*= "([^"]+)";.*/\1/')
swift_subprotocol=$(grep -E 'static let subprotocolV1 = ' "$SWIFT_CONST" | sed -E 's/.*= "([^"]+)".*/\1/')
expect_match "voice WS subprotocol v1" "$rust_subprotocol" "$swift_subprotocol"

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

# pause_sensitivity_ms::* — Swift consumes via Voice/VoiceReadiness.swift
# eouDelayMs(). Rust defines as canonical documentation/drift checkpoint.
rust_pause_tight=$(grep -E 'TIGHT: u32 =' "$RUST" | sed -E 's/.*= ([0-9]+);.*/\1/')
swift_pause_tight=$(grep -E 'static let tight: Int = ' "$SWIFT_CONST" | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "pause_sensitivity_ms::TIGHT" "$rust_pause_tight" "$swift_pause_tight"

rust_pause_balanced=$(grep -E 'BALANCED: u32 =' "$RUST" | sed -E 's/.*= ([0-9]+);.*/\1/')
swift_pause_balanced=$(grep -E 'static let balanced: Int = ' "$SWIFT_CONST" | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "pause_sensitivity_ms::BALANCED" "$rust_pause_balanced" "$swift_pause_balanced"

rust_pause_patient=$(grep -E 'PATIENT: u32 =' "$RUST" | sed -E 's/.*= ([0-9]+);.*/\1/')
swift_pause_patient=$(grep -E 'static let patient: Int = ' "$SWIFT_CONST" | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "pause_sensitivity_ms::PATIENT" "$rust_pause_patient" "$swift_pause_patient"

# Kokoro voice slot table (53 entries, 0..=52). The Rust `voice_id`
# function is authoritative — daemon rejects unknown ids; the Swift
# allSlots array drives the settings picker. Adding a voice to one side
# without the other gives the user a phantom option or hides a working
# one. We extract every quoted `xx_name` identifier from both files and
# diff them — same set, same count.
RUST_KOKORO="$ROOT/BoBeService/src/speech/providers/sherpa/kokoro_tts.rs"
SWIFT_KOKORO="$ROOT/BoBeMacUI/BoBe/Voice/VoiceSettingsEnums.swift"

rust_voices=$(grep -oE '"[a-z][a-z]_[a-z0-9]+"' "$RUST_KOKORO" | sort -u)
swift_voices=$(grep -oE '"[a-z][a-z]_[a-z0-9]+"' "$SWIFT_KOKORO" | sort -u)
rust_voice_count=$(echo "$rust_voices" | wc -l | tr -d ' ')
swift_voice_count=$(echo "$swift_voices" | wc -l | tr -d ' ')
expect_match "Kokoro voice slot count" "$rust_voice_count" "$swift_voice_count"
if [[ "$rust_voices" != "$swift_voices" ]]; then
    echo "drift: Kokoro voice set differs between Rust and Swift" >&2
    diff <(echo "$rust_voices") <(echo "$swift_voices") >&2 || true
    fail=1
fi

RUST_PROTO="$ROOT/BoBeService/src/speech/protocol.rs"
SWIFT_PROTO="$ROOT/BoBeMacUI/BoBe/Voice/VoiceProtocol.swift"

# Voice WS wire-type discriminators (SB11) + VoicePhase wire (SB12).
# Rust serializes enum variants via serde rename_all = "snake_case"; Swift
# encoder/decoder hand-writes the snake_case strings. If a Rust variant is
# added/renamed without the Swift side, the type tag stops round-tripping
# silently — daemon rejects the message or Swift drops a state update.
#
# Strategy: extract Rust variant identifiers (PascalCase) from each enum
# body, convert to snake_case; extract Swift wire literal strings from
# the encoder/decoder; diff.

pascal_to_snake() {
    # PascalCase → snake_case. Inserts `_` before each capital except the
    # first, then lowercases everything. `HelloAck` → `hello_ack`,
    # `BargeIn` → `barge_in`, `Idle` → `idle`.
    sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr '[:upper:]' '[:lower:]'
}

rust_enum_variants() {
    local file="$1"
    local enum_name="$2"
    # Slice the enum body and grep top-level variant identifiers (4-space
    # indent, capital first letter). Skips nested struct fields which are
    # 8-space indented and lowercase.
    awk "/pub\\(crate\\) enum $enum_name \\{/,/^}/" "$file" \
        | grep -oE '^    [A-Z][a-zA-Z0-9]+' \
        | sed -E 's/^    //' \
        | pascal_to_snake \
        | sort -u
}

diff_sets() {
    local name="$1" rust="$2" swift="$3"
    if [[ "$rust" != "$swift" ]]; then
        echo "drift: $name variant set differs between Rust and Swift" >&2
        diff <(echo "$rust") <(echo "$swift") >&2 || true
        fail=1
    fi
}

# VoicePhase (4 variants)
rust_phase=$(rust_enum_variants "$RUST_PROTO" VoicePhase)
swift_phase=$(awk '/enum VoicePhaseWire: String, Codable \{/,/^}/' "$SWIFT_PROTO" \
    | grep -oE 'case [a-z][a-zA-Z0-9]+' \
    | sed -E 's/case //' \
    | sort -u)
diff_sets "VoicePhase" "$rust_phase" "$swift_phase"

# ClientMessage (7 variants). Swift encoder hand-writes the type literal
# as `try c.encode("foo", forKey: .type)` — extract those strings.
rust_client=$(rust_enum_variants "$RUST_PROTO" ClientMessage)
swift_client=$(grep -oE 'c\.encode\("[a-z_]+", forKey: \.type\)' "$SWIFT_PROTO" \
    | sed -E 's/.*"([a-z_]+)".*/\1/' \
    | sort -u)
diff_sets "ClientMessage" "$rust_client" "$swift_client"

# ServerMessage (6 variants). Swift decoder switch arms are `case "foo":`
rust_server=$(rust_enum_variants "$RUST_PROTO" ServerMessage)
swift_server=$(awk '/enum ServerVoiceMessage: Decodable \{/,/^    \}/' "$SWIFT_PROTO" \
    | grep -oE 'case "[a-z_]+":' \
    | sed -E 's/case "([a-z_]+)":/\1/' \
    | sort -u)
diff_sets "ServerMessage" "$rust_server" "$swift_server"

# Main SSE event and indicator discriminators. Swift keeps an `unknown`
# compatibility case that has no daemon counterpart, so exclude it.
RUST_SSE="$ROOT/BoBeService/src/util/sse/types.rs"
SWIFT_SSE="$ROOT/BoBeMacUI/BoBe/DTOs/APITypes.swift"
SWIFT_STATE="$ROOT/BoBeMacUI/BoBe/Stores/BobeStoreState.swift"

rust_events=$(rust_enum_variants "$RUST_SSE" EventType)
swift_events=$(
    awk '/enum EventType: String, Codable, Sendable \{/,/^}/' "$SWIFT_SSE" \
        | sed -nE 's/^[[:space:]]*case ([a-zA-Z0-9_]+)( = "([^"]+)")?$/\1|\3/p' \
        | while IFS='|' read -r name wire; do
            [[ "$name" == "unknown" ]] && continue
            if [[ -n "$wire" ]]; then
                printf '%s\n' "$wire"
            else
                printf '%s\n' "$name" | pascal_to_snake
            fi
        done \
        | sort -u
)
diff_sets "SSE EventType" "$rust_events" "$swift_events"

rust_indicators=$(rust_enum_variants "$RUST_SSE" IndicatorType | tr '[:lower:]' '[:upper:]')
swift_indicators=$(
    awk '/enum IndicatorType: String, Codable, Sendable, Equatable \{/,/^}/' "$SWIFT_STATE" \
        | grep -oE '"[A-Z_]+"' \
        | tr -d '"' \
        | grep -v '^UNKNOWN$' \
        | sort -u
)
diff_sets "SSE IndicatorType" "$rust_indicators" "$swift_indicators"

# TTS binary frame header: 8B BE u64 chunk_id + 1B flags + N opus.
# Rust speech/protocol.rs defines the three; Swift Voice/VoiceProtocol.swift
# mirrors them. If either side adds a header byte or shifts a flag bit
# without the other, every TTS chunk silently misaligns at runtime.
# (RUST_PROTO / SWIFT_PROTO are defined earlier in the file.)

rust_header_len=$(grep -E 'TTS_FRAME_HEADER_LEN: usize =' "$RUST_PROTO" | sed -E 's/.*= ([0-9]+);.*/\1/')
swift_header_len=$(grep -E 'static let length = ' "$SWIFT_PROTO" | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "tts header length" "$rust_header_len" "$swift_header_len"

rust_flag_filler=$(grep -E 'FLAG_FILLER: u8 =' "$RUST_PROTO" | sed -E 's/.*= ([0-9b_]+);.*/\1/')
swift_flag_filler=$(grep -E 'static let flagFiller: UInt8 = ' "$SWIFT_PROTO" | sed -E 's/.*= ([0-9b_]+).*/\1/')
expect_match "tts FLAG_FILLER" "$rust_flag_filler" "$swift_flag_filler"

rust_flag_first=$(grep -E 'FLAG_FIRST_OF_TURN: u8 =' "$RUST_PROTO" | sed -E 's/.*= ([0-9b_]+);.*/\1/')
swift_flag_first=$(grep -E 'static let flagFirstOfTurn: UInt8 = ' "$SWIFT_PROTO" | sed -E 's/.*= ([0-9b_]+).*/\1/')
expect_match "tts FLAG_FIRST_OF_TURN" "$rust_flag_first" "$swift_flag_first"

if [[ $fail -eq 0 ]]; then
    echo "cross-language constants ok"
fi
exit $fail

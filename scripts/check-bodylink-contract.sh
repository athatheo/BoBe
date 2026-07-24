#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ESP_ROOT="${BOBE_ESP_TOY_ROOT:-$HOME/Repos/espBobeToy}"
RUST="$ROOT/BoBeService/src/body/protocol.rs"
SWIFT="$ROOT/BoBeMacUI/BoBe/BodyLink/BodyAdapterProtocol.swift"
SWIFT_ADAPTER="$ROOT/BoBeMacUI/BoBe/BodyLink/BodySpeechAdapter.swift"
C_HEADER="$ESP_ROOT/components/bobe_protocol/include/bobe_protocol.h"
C_TRANSPORT="$ESP_ROOT/components/bobe_transport/bobe_transport.c"
C_CONNECTIVITY="$ESP_ROOT/components/bobe_connectivity/bobe_connectivity.c"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bobe-bodylink-contract.XXXXXX")"
trap 'rm -rf "$TEMP_DIR"' EXIT

for file in \
    "$RUST" \
    "$SWIFT" \
    "$SWIFT_ADAPTER" \
    "$C_HEADER" \
    "$C_TRANSPORT" \
    "$C_CONNECTIVITY"; do
    if [[ ! -f "$file" ]]; then
        echo "BodyLink contract source missing: $file" >&2
        exit 1
    fi
done

fail=0
expect_match() {
    local name="$1"
    local left="$2"
    local right="$3"
    if [[ "$left" != "$right" ]]; then
        echo "BodyLink drift: $name — '$left' != '$right'" >&2
        fail=1
    fi
}

rust_number() {
    grep -E "const $1: [^=]+=" "$RUST" | sed -E 's/.*= ([0-9_]+);.*/\1/' | tr -d '_'
}
c_number() {
    grep -E "#define $1 " "$C_HEADER" | sed -E 's/.* ([0-9]+)U.*/\1/'
}
rust_string() {
    grep -E "const $1: &str =" "$RUST" | sed -E 's/.*"([^"]+)".*/\1/'
}
c_string() {
    grep -E "#define $1 " "$C_HEADER" | sed -E 's/.*"([^"]+)".*/\1/'
}

expect_match "protocol major" "$(rust_number PROTOCOL_MAJOR)" "$(c_number BOBE_PROTOCOL_MAJOR)"
expect_match "protocol minor" "$(rust_number PROTOCOL_MINOR)" "$(c_number BOBE_PROTOCOL_MINOR)"
expect_match "BodyLink media header" "$(rust_number MEDIA_HEADER_LEN)" "$(c_number BOBE_MEDIA_HEADER_BYTES)"
expect_match \
    "maximum playback credit" \
    "$(rust_number MAX_PLAYBACK_CREDIT_MS)" \
    "$(c_number BOBE_MAX_PLAYBACK_CREDIT_MS)"
expect_match \
    "maximum lease lifetime" \
    "$(rust_number MAX_LEASE_LIFETIME_MS)" \
    "$(c_number BOBE_MAX_LEASE_LIFETIME_MS)"
command -v cc >/dev/null
cat >"$TEMP_DIR/audio_sizes.c" <<'EOF'
#include "bobe_types.h"
#include <stdio.h>
int main(void) {
    printf(
        "%zu %zu\n",
        sizeof(int16_t) * (size_t)BOBE_MIC_FRAME_SAMPLES,
        sizeof(int16_t) * (size_t)BOBE_SPEAKER_FRAME_SAMPLES
    );
    return 0;
}
EOF
cc -std=c11 -Wall -Wextra -Werror \
    -I"$ESP_ROOT/components/bobe_common/include" \
    "$TEMP_DIR/audio_sizes.c" -o "$TEMP_DIR/audio_sizes"
read -r c_mic_pcm_bytes c_speaker_pcm_bytes < <("$TEMP_DIR/audio_sizes")
expect_match "microphone PCM bytes" "$(rust_number MIC_PCM_BYTES)" "$c_mic_pcm_bytes"
expect_match "speaker PCM bytes" "$(rust_number SPEAKER_PCM_BYTES)" "$c_speaker_pcm_bytes"

swift_adapter_header=$(awk '/struct BodyAdapterMediaHeader/,/^}/' "$SWIFT" \
    | grep -E 'static let length = ' | sed -E 's/.*= ([0-9]+).*/\1/')
expect_match "adapter media header" "$(rust_number ADAPTER_MEDIA_HEADER_LEN)" "$swift_adapter_header"

rust_body_protocol=$(grep -E 'BODY_SUBPROTOCOL_V1: &str =' "$RUST" | sed -E 's/.*"([^"]+)".*/\1/')
c_body_protocol=$(grep -E '\.subprotocol = ' "$C_TRANSPORT" | sed -E 's/.*"([^"]+)".*/\1/')
expect_match "body WS subprotocol" "$rust_body_protocol" "$c_body_protocol"
expect_match \
    "BodyLink mDNS auth" \
    "$(rust_string BODY_MDNS_AUTH)" \
    "$(c_string BOBE_BODY_MDNS_AUTH)"

if ! grep -Fq 'txt_matches(item, "auth", BOBE_BODY_MDNS_AUTH)' "$C_CONNECTIVITY"; then
    echo "BodyLink drift: ESP discovery does not enforce BOBE_BODY_MDNS_AUTH" >&2
    fail=1
fi

rust_adapter_protocol=$(grep -E 'ADAPTER_SUBPROTOCOL_V1: &str =' "$RUST" | sed -E 's/.*"([^"]+)".*/\1/')
swift_adapter_protocol=$(grep -E 'Sec-WebSocket-Protocol' "$SWIFT_ADAPTER" | sed -E 's/.*"([^"]+)", forHTTP.*/\1/')
expect_match "adapter WS subprotocol" "$rust_adapter_protocol" "$swift_adapter_protocol"

if [[ "$fail" -ne 0 ]]; then
    exit 1
fi
echo "BodyLink Rust/Swift/ESP contract constants match."

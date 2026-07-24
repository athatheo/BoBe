#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/lib/isolated-bobe-env.sh
source "$ROOT/scripts/lib/isolated-bobe-env.sh"
ESP_ROOT="${BOBE_ESP_TOY_ROOT:-$HOME/Repos/espBobeToy}"
MAIN_PORT="${BOBE_BODY_SMOKE_MAIN_PORT:-18777}"
BODY_PORT="${BOBE_BODY_SMOKE_DEVICE_PORT:-18778}"
ADAPTER_PORT="${BOBE_BODY_SMOKE_ADAPTER_PORT:-18779}"
MODEL="${BOBE_SMOKE_MODEL:-gpt-5.6-sol}"
REAL_ADAPTER="${BOBE_BODY_SMOKE_REAL_ADAPTER:-0}"
BINARY="$ROOT/BoBeService/target/release/bobe"
SWIFT_BINARY="$ROOT/BoBeMacUI/.build/release/BoBe"
ADAPTER_BINARY="$ROOT/BoBeMacUI/.build/release/BoBeBodyAdapter"
DATA_DIR="$(mktemp -d "$ESP_ROOT/private/bobe-body-smoke.XXXXXX")"
PKI_DIR="$DATA_DIR/pki"
LOG_FILE="$(mktemp "${TMPDIR:-/tmp}/bobe-body-smoke-log.XXXXXX")"
ADAPTER_LOG_FILE="$(mktemp "${TMPDIR:-/tmp}/bobe-body-adapter-log.XXXXXX")"
DEVICE_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
TRANSCRIPT="The blue lantern is ready for careful testing."
API_TOKEN=""
PID=""
ADAPTER_PID=""
STARTED=false
PURGE_SUCCEEDED=false
SUCCEEDED=false
PRIVACY_TIMEOUT_SECS=60

listener_pids() {
    local port="$1"
    lsof -nP -a -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null | sort -u || true
}

all_ports_owned_by_child() {
    [[ -n "$PID" ]] \
        && [[ "$(listener_pids "$MAIN_PORT")" == "$PID" ]] \
        && [[ "$(listener_pids "$BODY_PORT")" == "$PID" ]] \
        && [[ "$(listener_pids "$ADAPTER_PORT")" == "$PID" ]]
}

main_owned_by_child() {
    [[ -n "$PID" ]] && [[ "$(listener_pids "$MAIN_PORT")" == "$PID" ]]
}

authenticated_curl() {
    curl --header "Authorization: Bearer $API_TOKEN" "$@"
}

main_request() {
    if ! main_owned_by_child; then
        echo "Refusing request: main port is not owned by BodyLink smoke PID $PID" >&2
        return 1
    fi
    authenticated_curl "$@"
}

wait_until_accepting() {
    for _ in $(seq 1 600); do
        local status
        status="$(main_request --silent --fail --max-time 1 \
            "http://127.0.0.1:$MAIN_PORT/status" || true)"
        if [[ "$status" == *'"accepting_user_messages":true'* ]]; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}

cleanup() {
    if [[ "$STARTED" == true && "$PURGE_SUCCEEDED" == false ]] \
        && [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null \
        && main_owned_by_child \
        && bobe_process_uses_database "$PID" "$DATA_DIR/data/bobrust.db"; then
        wait_until_accepting 2>/dev/null || true
        if main_request --silent --fail --max-time "$PRIVACY_TIMEOUT_SECS" \
            -X DELETE "http://127.0.0.1:$MAIN_PORT/privacy/data" >/dev/null; then
            PURGE_SUCCEEDED=true
        fi
    fi
    if [[ -n "$ADAPTER_PID" ]] && kill -0 "$ADAPTER_PID" 2>/dev/null; then
        kill "$ADAPTER_PID"
        for _ in $(seq 1 100); do
            kill -0 "$ADAPTER_PID" 2>/dev/null || break
            sleep 0.05
        done
        if kill -0 "$ADAPTER_PID" 2>/dev/null; then
            kill -9 "$ADAPTER_PID"
        fi
        wait "$ADAPTER_PID" 2>/dev/null || true
    fi
    rm -f "$ADAPTER_BINARY"
    if [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null; then
        kill "$PID"
        for _ in $(seq 1 100); do
            kill -0 "$PID" 2>/dev/null || break
            sleep 0.05
        done
        if kill -0 "$PID" 2>/dev/null; then
            kill -9 "$PID"
        fi
        wait "$PID" 2>/dev/null || true
    fi
    if [[ "$STARTED" == false || "$SUCCEEDED" == true ]]; then
        rm -rf "$DATA_DIR"
        rm -f "$LOG_FILE" "$ADAPTER_LOG_FILE"
    else
        echo "BodyLink smoke data preserved at: $DATA_DIR" >&2
        echo "BodyLink daemon log preserved at: $LOG_FILE" >&2
        if [[ "$REAL_ADAPTER" == "1" ]]; then
            echo "BodyLink Swift adapter log preserved at: $ADAPTER_LOG_FILE" >&2
        fi
    fi
}
trap cleanup EXIT

for command in curl lsof openssl pnpm sqlite3; do
    command -v "$command" >/dev/null
done
if [[ "$REAL_ADAPTER" == "1" ]]; then
    for command in afconvert say; do
        command -v "$command" >/dev/null
    done
    if [[ ! -x "$SWIFT_BINARY" ]]; then
        echo "Release Swift app missing: $SWIFT_BINARY" >&2
        exit 1
    fi
fi
if [[ ! -x "$BINARY" ]]; then
    echo "Release daemon missing: $BINARY" >&2
    exit 1
fi
if [[ ! -x "$ESP_ROOT/tools/generate_bodylink_pki.sh" ]]; then
    echo "ESP BodyLink PKI generator missing under $ESP_ROOT" >&2
    exit 1
fi
for port in "$MAIN_PORT" "$BODY_PORT" "$ADAPTER_PORT"; do
    if [[ ! "$port" =~ ^[0-9]+$ ]] || (( port < 1 || port > 65535 )); then
        echo "Invalid BodyLink smoke port: $port" >&2
        exit 1
    fi
    occupied="$(listener_pids "$port")"
    if [[ -n "$occupied" ]]; then
        echo "BodyLink smoke port $port is already owned by PID(s): $occupied" >&2
        exit 1
    fi
done

pnpm --dir "$ROOT/scripts/bodylink-sim" install --frozen-lockfile >/dev/null
"$ESP_ROOT/tools/generate_bodylink_pki.sh" \
    "$PKI_DIR" "$DEVICE_ID" localhost "$BODY_PORT" >/dev/null
openssl genpkey -algorithm EC \
    -pkeyopt ec_paramgen_curve:P-256 \
    -out "$PKI_DIR/rogue-device-key.pem"
openssl req -new \
    -key "$PKI_DIR/rogue-device-key.pem" \
    -subj "/CN=rogue-body" \
    -out "$PKI_DIR/rogue-device.csr"
printf 'extendedKeyUsage=clientAuth\nsubjectAltName=URI:urn:bobe:body:rogue-body\n' \
    >"$PKI_DIR/rogue-device.ext"
openssl x509 -req -sha256 -days 30 \
    -in "$PKI_DIR/rogue-device.csr" \
    -CA "$PKI_DIR/ca-cert.pem" \
    -CAkey "$PKI_DIR/ca-key.pem" \
    -CAcreateserial \
    -extfile "$PKI_DIR/rogue-device.ext" \
    -out "$PKI_DIR/rogue-device-cert.pem" >/dev/null
# shellcheck disable=SC1091
source "$PKI_DIR/bobe-body.env"

API_TOKEN="$(openssl rand -hex 32)"
ln -s "$HOME/.bobe/models" "$DATA_DIR/models"

"${BOBE_ISOLATED_ENV[@]}" \
    BOBE_DATA_DIR="$DATA_DIR" \
    BOBE_SERVER__PORT="$MAIN_PORT" \
    BOBE_SERVER__API_TOKEN="$API_TOKEN" \
    BOBE_CAPTURE__ENABLED=false \
    BOBE_CHECKIN__ENABLED=false \
    BOBE_SEED_DEFAULT_DOCUMENTS=false \
    BOBE_ENGINE__PROVIDER_CHAT_MODEL="$MODEL" \
    BOBE_VOICE__ENABLED=true \
    BOBE_BODY__ENABLED=true \
    BOBE_BODY__HOST=127.0.0.1 \
    BOBE_BODY__ADVERTISED_HOSTNAME="$BOBE_BODY__ADVERTISED_HOSTNAME" \
    BOBE_BODY__ENROLLED_DEVICE_ID="$BOBE_BODY__ENROLLED_DEVICE_ID" \
    BOBE_BODY__PORT="$BODY_PORT" \
    BOBE_BODY__ADAPTER_PORT="$ADAPTER_PORT" \
    BOBE_BODY__ADAPTER_TOKEN="$BOBE_BODY__ADAPTER_TOKEN" \
    BOBE_BODY__TLS_CERT_PATH="$BOBE_BODY__TLS_CERT_PATH" \
    BOBE_BODY__TLS_KEY_PATH="$BOBE_BODY__TLS_KEY_PATH" \
    BOBE_BODY__CLIENT_CA_PATH="$BOBE_BODY__CLIENT_CA_PATH" \
    BOBE_BODY__ENROLLED_CLIENT_CERT_PATH="$BOBE_BODY__ENROLLED_CLIENT_CERT_PATH" \
    BOBE_BODY__TRUST_KEY_ID="$BOBE_BODY__TRUST_KEY_ID" \
    BOBE_BODY__CONTROLLER_EPOCH="$BOBE_BODY__CONTROLLER_EPOCH" \
    BOBE_BODY__MDNS_ENABLED=false \
    "$BINARY" serve >"$LOG_FILE" 2>&1 &
PID=$!
STARTED=true

for _ in $(seq 1 600); do
    if ! kill -0 "$PID" 2>/dev/null; then
        cat "$LOG_FILE" >&2
        exit 1
    fi
    if authenticated_curl --silent --fail --max-time 1 \
        "http://127.0.0.1:$MAIN_PORT/health" >/dev/null \
        && all_ports_owned_by_child; then
        break
    fi
    sleep 0.05
done
if ! all_ports_owned_by_child; then
    echo "BodyLink daemon did not own all three listeners" >&2
    cat "$LOG_FILE" >&2
    exit 1
fi

for _ in $(seq 1 500); do
    grep -q 'bootstrap\.chat_prewarm\.ok' "$LOG_FILE" && break
    grep -q 'bootstrap\.chat_prewarm\.failed' "$LOG_FILE" && {
        cat "$LOG_FILE" >&2
        exit 1
    }
    sleep 0.05
done
grep -q 'bootstrap\.chat_prewarm\.ok' "$LOG_FILE"

WAV_PATH=""
if [[ "$REAL_ADAPTER" == "1" ]]; then
    say -v Samantha -o "$DATA_DIR/body-input.aiff" "$TRANSCRIPT"
    WAV_PATH="$DATA_DIR/body-input.wav"
    afconvert "$DATA_DIR/body-input.aiff" "$WAV_PATH" \
        -f WAVE -d LEI16@16000 -c 1
    ln -f "$SWIFT_BINARY" "$ADAPTER_BINARY"
    "${BOBE_ISOLATED_ENV[@]}" \
        BOBE_BODY_ADAPTER_ONLY=1 \
        BOBE_BODY_ADAPTER=1 \
        BOBE_BODY_ADAPTER_URL="http://127.0.0.1:$ADAPTER_PORT" \
        BOBE_BODY__ADAPTER_TOKEN="$BOBE_BODY__ADAPTER_TOKEN" \
        "$ADAPTER_BINARY" >"$ADAPTER_LOG_FILE" 2>&1 &
    ADAPTER_PID=$!
    for _ in $(seq 1 1200); do
        if ! kill -0 "$ADAPTER_PID" 2>/dev/null; then
            cat "$ADAPTER_LOG_FILE" >&2
            exit 1
        fi
        grep -q 'body.adapter_connected' "$LOG_FILE" && break
        sleep 0.1
    done
    grep -q 'body.adapter_connected' "$LOG_FILE"
fi

BOBE_SIM_MAIN_URL="http://127.0.0.1:$MAIN_PORT" \
BOBE_SIM_MAIN_TOKEN="$API_TOKEN" \
BOBE_SIM_BODY_URL="wss://127.0.0.1:$BODY_PORT/body/v1" \
BOBE_SIM_ADAPTER_URL="ws://127.0.0.1:$ADAPTER_PORT/body/adapter" \
BOBE_SIM_ADAPTER_TOKEN="$BOBE_BODY__ADAPTER_TOKEN" \
BOBE_SIM_DEVICE_ID="$DEVICE_ID" \
BOBE_SIM_CA_PATH="$PKI_DIR/ca-cert.pem" \
BOBE_SIM_CERT_PATH="$PKI_DIR/device-cert.pem" \
BOBE_SIM_KEY_PATH="$PKI_DIR/device-key.pem" \
BOBE_SIM_ROGUE_CERT_PATH="$PKI_DIR/rogue-device-cert.pem" \
BOBE_SIM_ROGUE_KEY_PATH="$PKI_DIR/rogue-device-key.pem" \
BOBE_SIM_TRANSCRIPT="$TRANSCRIPT" \
BOBE_SIM_EXTERNAL_ADAPTER="$REAL_ADAPTER" \
BOBE_SIM_WAV_PATH="$WAV_PATH" \
    pnpm --dir "$ROOT/scripts/bodylink-sim" exec node simulator.mjs

if [[ "$REAL_ADAPTER" == "1" ]] && ! kill -0 "$ADAPTER_PID" 2>/dev/null; then
    echo "BodyLink speech adapter exited after a completed turn" >&2
    cat "$ADAPTER_LOG_FILE" >&2
    exit 1
fi

wait_until_accepting
database="$DATA_DIR/data/bobrust.db"
if ! bobe_process_uses_database "$PID" "$database"; then
    echo "Refusing purge: BodyLink daemon is not using $database" >&2
    exit 1
fi
if [[ "$REAL_ADAPTER" == "1" ]]; then
    user_turns="$(sqlite3 "$database" \
        "SELECT COUNT(*) FROM conversation_turns WHERE role = 'user' AND length(content) > 0;")"
    if [[ "$user_turns" -lt 1 ]]; then
        echo "Swift adapter produced no persisted transcript" >&2
        exit 1
    fi
else
    sqlite3 "$database" \
        "SELECT content FROM conversation_turns WHERE role = 'user';" \
        | grep -F -q "$TRANSCRIPT"
fi
assistant_turns="$(sqlite3 "$database" \
    "SELECT COUNT(*) FROM conversation_turns WHERE role = 'assistant' AND length(content) > 0;")"
if [[ "$assistant_turns" -lt 1 ]]; then
    echo "BodyLink turn did not persist an assistant response" >&2
    exit 1
fi

main_request --silent --fail --max-time "$PRIVACY_TIMEOUT_SECS" \
    -X DELETE "http://127.0.0.1:$MAIN_PORT/privacy/data" >/dev/null
PURGE_SUCCEEDED=true

remaining_rows="$(sqlite3 "$database" \
    'SELECT (SELECT COUNT(*) FROM conversations) + (SELECT COUNT(*) FROM conversation_turns);')"
if [[ "$remaining_rows" != "0" ]]; then
    echo "BodyLink privacy purge left $remaining_rows conversation row(s)" >&2
    exit 1
fi

printf '{\n'
printf '  "bodylink_transport": "ok",\n'
printf '  "targeted_text_audio": "ok",\n'
printf '  "legacy_sse_privacy": "ok",\n'
printf '  "conversation_persistence": "ok",\n'
printf '  "privacy_purge": "ok"\n'
printf '}\n'
SUCCEEDED=true

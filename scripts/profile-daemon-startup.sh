#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/lib/isolated-bobe-env.sh
source "$ROOT/scripts/lib/isolated-bobe-env.sh"
PORT="${BOBE_PROFILE_PORT:-18766}"
PRIVACY_TIMEOUT_SECS=60
BINARY="$ROOT/BoBeService/target/release/bobe"
DATA_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bobe-profile.XXXXXX")"
LOG_FILE="$(mktemp "${TMPDIR:-/tmp}/bobe-profile-log.XXXXXX")"
PID=""
STARTED=false
PURGE_SUCCEEDED=false
API_TOKEN=""

listener_pids() {
    lsof -nP -a -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null | sort -u || true
}

listener_owned_by_child() {
    [[ -n "$PID" ]] && [[ "$(listener_pids)" == "$PID" ]]
}

authenticated_curl() {
    curl --header "Authorization: Bearer $API_TOKEN" "$@"
}

bobe_request() {
    if ! listener_owned_by_child; then
        echo "Refusing request: port $PORT is not owned by profile daemon PID $PID" >&2
        return 1
    fi
    authenticated_curl "$@"
}

now_ms() {
    perl -MTime::HiRes=time -e 'printf "%.0f\n", time() * 1000'
}

cleanup() {
    if [[ "$STARTED" == true && "$PURGE_SUCCEEDED" == false ]] \
        && [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null \
        && listener_owned_by_child \
        && bobe_process_uses_database "$PID" "$DATA_DIR/data/bobrust.db"; then
        if bobe_request --silent --fail --max-time "$PRIVACY_TIMEOUT_SECS" -X DELETE \
            "http://127.0.0.1:$PORT/privacy/data" >/dev/null; then
            PURGE_SUCCEEDED=true
        fi
    fi
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
    if [[ "$STARTED" == false || "$PURGE_SUCCEEDED" == true ]]; then
        rm -rf "$DATA_DIR"
        rm -f "$LOG_FILE"
    else
        echo "Profile cleanup preserved for purge retry: $DATA_DIR" >&2
        echo "Daemon log preserved at: $LOG_FILE" >&2
    fi
}
trap cleanup EXIT

if [[ ! -x "$BINARY" ]]; then
    echo "Release daemon missing: $BINARY" >&2
    echo "Run: just build-backend" >&2
    exit 1
fi
if [[ ! "$PORT" =~ ^[0-9]+$ ]] || (( PORT < 1 || PORT > 65535 )); then
    echo "BOBE_PROFILE_PORT must be an integer from 1 through 65535" >&2
    exit 1
fi
command -v curl >/dev/null
command -v lsof >/dev/null
command -v openssl >/dev/null
occupied_pids="$(listener_pids)"
if [[ -n "$occupied_pids" ]]; then
    echo "BOBE_PROFILE_PORT $PORT is already owned by PID(s): $occupied_pids" >&2
    exit 1
fi
API_TOKEN="$(openssl rand -hex 32)"

start_ms="$(now_ms)"
"${BOBE_ISOLATED_ENV[@]}" \
    BOBE_DATA_DIR="$DATA_DIR" \
    BOBE_SERVER__PORT="$PORT" \
    BOBE_SERVER__API_TOKEN="$API_TOKEN" \
    BOBE_CAPTURE__ENABLED=false \
    BOBE_SEED_DEFAULT_DOCUMENTS=false \
    "$BINARY" serve >"$LOG_FILE" 2>&1 &
PID=$!
STARTED=true

health_ms=""
for _ in $(seq 1 600); do
    if ! kill -0 "$PID" 2>/dev/null; then
        cat "$LOG_FILE" >&2
        exit 1
    fi
    if authenticated_curl --silent --fail --max-time 1 \
        "http://127.0.0.1:$PORT/health" >/dev/null; then
        if ! listener_owned_by_child; then
            echo "Health response on port $PORT did not come from profile daemon PID $PID" >&2
            cat "$LOG_FILE" >&2
            exit 1
        fi
        health_ms="$(now_ms)"
        break
    fi
    sleep 0.05
done

if [[ -z "$health_ms" ]]; then
    echo "Daemon did not become healthy within 30 seconds" >&2
    cat "$LOG_FILE" >&2
    exit 1
fi

prewarm_ms=""
prewarm_status=""
for _ in $(seq 1 400); do
    if prewarm_status="$(
        grep -oE 'bootstrap\.chat_prewarm\.(ok|failed)' "$LOG_FILE" \
            | tail -n 1 \
            | sed 's/.*\.//'
    )" && [[ -n "$prewarm_status" ]]; then
        prewarm_ms="$(now_ms)"
        break
    fi
    sleep 0.05
done

binary_bytes="$(stat -f '%z' "$BINARY")"
rss_kb="$(ps -o rss= -p "$PID" | tr -d ' ')"
health_elapsed_ms=$((health_ms - start_ms))

# The prewarm creates a persisted Copilot session. Delete it through BoBe's
# real privacy path before removing the temporary ID files, or repeated
# profiles would leave unreachable SDK sessions behind.
if ! bobe_process_uses_database "$PID" "$DATA_DIR/data/bobrust.db"; then
    echo "Refusing purge: profile daemon is not using its temporary database" >&2
    exit 1
fi
bobe_request --silent --fail --max-time "$PRIVACY_TIMEOUT_SECS" -X DELETE \
    "http://127.0.0.1:$PORT/privacy/data" >/dev/null
PURGE_SUCCEEDED=true

printf '{\n'
printf '  "health_ready_ms": %s,\n' "$health_elapsed_ms"
if [[ -n "$prewarm_ms" ]]; then
    printf '  "copilot_prewarm_ms": %s,\n' "$((prewarm_ms - start_ms))"
    printf '  "copilot_prewarm_status": "%s",\n' "$prewarm_status"
else
    printf '  "copilot_prewarm_ms": null,\n'
    printf '  "copilot_prewarm_status": "timeout",\n'
fi
printf '  "daemon_rss_kb": %s,\n' "$rss_kb"
printf '  "daemon_bytes": %s\n' "$binary_bytes"
printf '}\n'

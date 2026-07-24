#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/lib/isolated-bobe-env.sh
source "$ROOT/scripts/lib/isolated-bobe-env.sh"
PORT="${BOBE_SMOKE_PORT:-18776}"
MODEL="${BOBE_SMOKE_MODEL:-gpt-5.6-sol}"
PRIVACY_TIMEOUT_SECS=60
BINARY="$ROOT/BoBeService/target/release/bobe"
DATA_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bobe-tool-smoke.XXXXXX")"
LOG_FILE="$(mktemp "${TMPDIR:-/tmp}/bobe-tool-smoke-log.XXXXXX")"
RUN_ID="smoke-$(date +%s)-$$"
GOAL_TITLE="E2E Goal $RUN_ID"
GOAL_WORK_MARKER="work-$RUN_ID"
GOAL_QUESTION_MARKER="question-$RUN_ID"
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
        echo "Refusing request: port $PORT is not owned by smoke daemon PID $PID" >&2
        return 1
    fi
    authenticated_curl "$@"
}

cleanup() {
    if [[ "$STARTED" == true && "$PURGE_SUCCEEDED" == false ]] \
        && [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null \
        && listener_owned_by_child \
        && bobe_process_uses_database "$PID" "$DATA_DIR/data/bobrust.db"; then
        wait_until_accepting 2>/dev/null || true
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
        echo "Smoke data preserved for diagnosis: $DATA_DIR" >&2
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
    echo "BOBE_SMOKE_PORT must be an integer from 1 through 65535" >&2
    exit 1
fi
if [[ ! "$MODEL" =~ ^[A-Za-z0-9._-]+$ ]]; then
    echo "BOBE_SMOKE_MODEL contains unsupported characters" >&2
    exit 1
fi
command -v curl >/dev/null
command -v lsof >/dev/null
command -v openssl >/dev/null
command -v sqlite3 >/dev/null
occupied_pids="$(listener_pids)"
if [[ -n "$occupied_pids" ]]; then
    echo "BOBE_SMOKE_PORT $PORT is already owned by PID(s): $occupied_pids" >&2
    exit 1
fi
API_TOKEN="$(openssl rand -hex 32)"

wait_until_accepting() {
    for _ in $(seq 1 450); do
        status="$(bobe_request --silent --fail --max-time 1 \
            "http://127.0.0.1:$PORT/status" || true)"
        if [[ "$status" == *'"accepting_user_messages":true'* ]]; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}

"${BOBE_ISOLATED_ENV[@]}" \
    BOBE_DATA_DIR="$DATA_DIR" \
    BOBE_SERVER__PORT="$PORT" \
    BOBE_SERVER__API_TOKEN="$API_TOKEN" \
    BOBE_CAPTURE__ENABLED=false \
    BOBE_SEED_DEFAULT_DOCUMENTS=false \
    BOBE_ENGINE__PROVIDER_CHAT_MODEL="$MODEL" \
    "$BINARY" serve >"$LOG_FILE" 2>&1 &
PID=$!
STARTED=true

for _ in $(seq 1 600); do
    if ! kill -0 "$PID" 2>/dev/null; then
        cat "$LOG_FILE" >&2
        exit 1
    fi
    if authenticated_curl --silent --fail --max-time 1 \
        "http://127.0.0.1:$PORT/health" >/dev/null; then
        if ! listener_owned_by_child; then
            echo "Health response on port $PORT did not come from smoke daemon PID $PID" >&2
            cat "$LOG_FILE" >&2
            exit 1
        fi
        break
    fi
    sleep 0.05
done
bobe_request --silent --fail --max-time 1 \
    "http://127.0.0.1:$PORT/health" >/dev/null

for _ in $(seq 1 400); do
    if grep -q 'bootstrap\.chat_prewarm\.ok' "$LOG_FILE"; then
        break
    fi
    if grep -q 'bootstrap\.chat_prewarm\.failed' "$LOG_FILE"; then
        cat "$LOG_FILE" >&2
        exit 1
    fi
    sleep 0.05
done
grep -q 'bootstrap\.chat_prewarm\.ok' "$LOG_FILE"

memory_body="$(printf \
    '{"content":"Remember this durable smoke-test marker: %s. Use bobe_memory_append in long_term, then confirm briefly."}' \
    "$RUN_ID")"
bobe_request --silent --fail --max-time 10 \
    -H 'Content-Type: application/json' \
    -d "$memory_body" \
    "http://127.0.0.1:$PORT/message" >/dev/null

for _ in $(seq 1 450); do
    if grep -q "$RUN_ID" "$DATA_DIR/memory.md" 2>/dev/null; then
        break
    fi
    sleep 0.1
done
grep -q "$RUN_ID" "$DATA_DIR/memory.md"
wait_until_accepting

goal_body="$(printf \
    '{"content":"Yes, track a goal named %s with summary Verify domain tool wiring, why it matters E2E confidence, priority 3. Use bobe_goal_create."}' \
    "$GOAL_TITLE")"
bobe_request --silent --fail --max-time 10 \
    -H 'Content-Type: application/json' \
    -d "$goal_body" \
    "http://127.0.0.1:$PORT/message" >/dev/null

for _ in $(seq 1 450); do
    if grep -R -q "^# $GOAL_TITLE$" "$DATA_DIR/goals" 2>/dev/null; then
        break
    fi
    sleep 0.1
done
grep -R -q "^# $GOAL_TITLE$" "$DATA_DIR/goals"
wait_until_accepting

goal_update_body="$(printf \
    '{"content":"Update the goal named %s. First use bobe_goal_list, then bobe_goal_update. Set how_working_on_it to exactly %s and open_questions to exactly %s. Confirm briefly."}' \
    "$GOAL_TITLE" "$GOAL_WORK_MARKER" "$GOAL_QUESTION_MARKER")"
bobe_request --silent --fail --max-time 10 \
    -H 'Content-Type: application/json' \
    -d "$goal_update_body" \
    "http://127.0.0.1:$PORT/message" >/dev/null

for _ in $(seq 1 450); do
    if grep -R -q "$GOAL_WORK_MARKER" "$DATA_DIR/goals" 2>/dev/null \
        && grep -R -q "$GOAL_QUESTION_MARKER" "$DATA_DIR/goals" 2>/dev/null; then
        break
    fi
    sleep 0.1
done
grep -R -q "$GOAL_WORK_MARKER" "$DATA_DIR/goals"
grep -R -q "$GOAL_QUESTION_MARKER" "$DATA_DIR/goals"
wait_until_accepting

if ! bobe_process_uses_database "$PID" "$DATA_DIR/data/bobrust.db"; then
    echo "Refusing purge: smoke daemon is not using its temporary database" >&2
    exit 1
fi
bobe_request --silent --fail --max-time "$PRIVACY_TIMEOUT_SECS" -X DELETE \
    "http://127.0.0.1:$PORT/privacy/data" >/dev/null
PURGE_SUCCEEDED=true

test ! -d "$DATA_DIR/workers"
if grep -q "$RUN_ID" "$DATA_DIR/memory.md" 2>/dev/null; then
    echo "Privacy purge left the memory marker behind" >&2
    exit 1
fi
if grep -R -q "^# $GOAL_TITLE$" "$DATA_DIR/goals" 2>/dev/null; then
    echo "Privacy purge left the smoke goal behind" >&2
    exit 1
fi
remaining_rows="$(sqlite3 "$DATA_DIR/data/bobrust.db" \
    'SELECT (SELECT COUNT(*) FROM conversations) + (SELECT COUNT(*) FROM conversation_turns);')"
if [[ "$remaining_rows" != "0" ]]; then
    echo "Privacy purge left $remaining_rows conversation row(s) behind" >&2
    exit 1
fi

printf '{\n'
printf '  "copilot_prewarm": "ok",\n'
printf '  "memory_tool": "ok",\n'
printf '  "goal_create_tool": "ok",\n'
printf '  "goal_update_tool": "ok",\n'
printf '  "privacy_purge": "ok"\n'
printf '}\n'

#!/usr/bin/env bash

# Preserve authentication and toolchain variables while preventing a caller's
# BOBE_* overrides from redirecting an isolated daemon into real user storage.
BOBE_ISOLATED_ENV=(env)
while IFS='=' read -r name _; do
    case "$name" in
        BOBE_*) BOBE_ISOLATED_ENV+=(-u "$name") ;;
    esac
done < <(env)

bobe_process_uses_database() {
    local pid="$1"
    local database="$2"
    [[ -f "$database" ]] &&
        [[ "$(lsof -nP -a -p "$pid" -t "$database" 2>/dev/null | sort -u)" == "$pid" ]]
}

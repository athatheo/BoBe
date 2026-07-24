#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CATALOG_ROOT="$ROOT/BoBeMacUI/BoBe/Resources/i18n"
ENGLISH="$CATALOG_ROOT/en.lproj/UI.strings"
SOURCE_ROOT="$ROOT/BoBeMacUI/BoBe"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bobe-localization-check.XXXXXX")"
trap 'rm -rf "$TEMP_DIR"' EXIT

extract_keys() {
    sed -nE 's/^[[:space:]]*"([^"]+)"[[:space:]]*=.*/\1/p' "$1"
}

check_catalog() {
    local catalog="$1"
    local locale
    locale="$(basename "$(dirname "$catalog")")"
    local raw="$TEMP_DIR/$locale.raw"
    local sorted="$TEMP_DIR/$locale.keys"

    plutil -lint "$catalog" >/dev/null
    extract_keys "$catalog" >"$raw"
    LC_ALL=C sort "$raw" >"$sorted"

    local duplicates
    duplicates="$(uniq -d "$sorted")"
    if [[ -n "$duplicates" ]]; then
        echo "$locale contains duplicate localization keys:" >&2
        printf '%s\n' "$duplicates" >&2
        return 1
    fi

    if [[ "$catalog" != "$ENGLISH" ]]; then
        local unknown
        unknown="$(comm -23 "$sorted" "$TEMP_DIR/en.lproj.keys")"
        if [[ -n "$unknown" ]]; then
            echo "$locale contains keys absent from the English source catalog:" >&2
            printf '%s\n' "$unknown" >&2
            return 1
        fi
    fi
}

check_catalog "$ENGLISH"
while IFS= read -r catalog; do
    [[ "$catalog" == "$ENGLISH" ]] && continue
    check_catalog "$catalog"
done < <(find "$CATALOG_ROOT" -name UI.strings -type f | LC_ALL=C sort)

rg --pcre2 --multiline --no-filename --only-matching --replace '$1' \
    '(?:L10n\.tr|String\s*\(\s*localized:|LocalizedStringKey|Text|Button|Label|Toggle|\.navigationTitle|\.help)\s*\(\s*"([a-z0-9_]+(?:\.[a-z0-9_]+)+)"' \
    "$SOURCE_ROOT" --glob '*.swift' \
    | LC_ALL=C sort -u >"$TEMP_DIR/source.keys"

missing="$(comm -23 "$TEMP_DIR/source.keys" "$TEMP_DIR/en.lproj.keys")"
if [[ -n "$missing" ]]; then
    echo "Swift source references localization keys absent from the English catalog:" >&2
    printf '%s\n' "$missing" >&2
    exit 1
fi

echo "Localization catalogs and static Swift references are valid."

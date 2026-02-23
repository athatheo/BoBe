# Terminal & CLI Tools (macOS + fish)

Date captured: 2026-01-26

This document describes the machine, the fish shell behaviors that affect automation, and the CLI tools available for scripting.

## Machine

- Computer name: John’s MacBook Pro
- OS: macOS 26.2 (Build 25C56)
- Kernel: Darwin 25.2.0
- CPU: Apple M4 Pro (14 cores)
- Arch: arm64
- Memory: 24 GB

## Shell: fish

- Default shell: fish 4.3.3 (`/opt/homebrew/bin/fish`)
- Config files:
  - `~/.config/fish/config.fish`
  - `~/.config/fish/conf.d/05-fastfetch.fish`
  - `~/.config/fish/conf.d/fish_frozen_key_bindings.fish`

### Important interactive behaviors

- `cat` is aliased to `bat --paging=never`.
  - For raw output, use `command cat`.
- `grep` is aliased to `rg`.
  - For BSD grep, use `command grep`.
- `cd` is wrapped by zoxide (`zoxide init fish --cmd cd`).
  - For builtin semantics, use `builtin cd`.

### Scripting guidelines (fish-specific)

- Fish does **not** support bash-style heredocs (`<<EOF`).
  - For multiline content in fish, prefer `printf` or `string join`.
  - If you truly need heredocs, run a bash subshell.

Examples:

```fish
# Write a small file (fish-native)
printf '%s\n' 'line1' 'line2' > /tmp/example.txt

# Bypass aliases in scripts
command cat /tmp/example.txt
command grep -n 'line' /tmp/example.txt

# If a snippet assumes bash syntax, run it explicitly
bash -lc "cat <<'EOF' > /tmp/example2.txt
hello
EOF"
```

Notes:

- Prefer non-interactive fish (`fish -c '...'`) for automation. Avoid `fish -ic` unless you need interactive init.
- If you must use `fish -ic`, suppress the fastfetch banner with `__FASTFETCH_SHOWN=1`.

## Package manager: Homebrew

- Prefix: `/opt/homebrew`
- Homebrew: 5.0.11

Agent-focused discovery commands:

- `brew leaves` (top-level installs)
- `brew list --formula`
- `brew list --cask`

## Tools available (agent-relevant)

### Common automation tools

- `jq` (JSON filtering/query)
- `xh` (HTTP client)
- `ripgrep (rg)` (fast text search)
- `fd` (fast file discovery)
- `git` (Apple Git) + `gh` (GitHub CLI) + `glab` (GitLab CLI)
- `docker` + `docker-compose`
- `azure` (Azure CLI)
- `openssh` (newer SSH from Homebrew)
- `mkcert` (local TLS certs)

### GitHub CLI (`gh`) for automated agents

Auth verification (read-only, safe):

```sh
gh auth status -t
gh api user --jq .login
gh api rate_limit --jq '.resources.core | {limit, remaining, reset}'
```

Automation tips:

- Prefer structured output:
  - `gh repo view OWNER/REPO --json name,owner,defaultBranchRef`
  - `gh pr list --repo OWNER/REPO --json number,title,state,author --limit 50`
- Avoid paging in agent runs:
  - Set `GH_PAGER=cat` (and optionally `PAGER=cat`).
- Avoid ANSI/color noise:
  - Use `NO_COLOR=1` where possible; for git use `git -c color.ui=false ...`.
- Headless/CI-style auth:
  - Provide `GH_TOKEN` in the process environment (least privilege; org SSO must be authorized if applicable).
  - Avoid printing tokens. Do not run `gh auth token` in logs.

### PDF tooling (Poppler)

`poppler` is a PDF rendering/conversion toolkit. On this machine it provides:

- `pdftotext` (extract PDF text)
- `pdfinfo` (PDF metadata)

Useful patterns:

```sh
pdftotext -layout input.pdf -
pdfinfo input.pdf
```

### Language toolchains

#### Node.js

- `node v25.4.0`, `npm 11.7.0`
- `nvm` is installed via Homebrew but is not a standalone binary in PATH (it usually requires sourcing `nvm.sh`).

#### Python

- `python3` → Python 3.14.2
- `python3.12` → Python 3.12.12
- Tools: `pyenv`, `uv`

#### .NET

- `DOTNET_ROOT=/usr/local/share/dotnet`
- Cask: `dotnet-sdk`

#### Go

- `go` (Homebrew)

#### Swift/Xcode

- `swiftlint`, `swiftformat`, `xcbeautify`, `xcodegen`

## Human UX tools (installed, optional)

These are useful for interactive terminals but generally not needed by agents:

- `fastfetch` (system banner). Runs once per interactive fish session.
- `fzf`, `btop`, `eza`, `bat`, `atuin`, `zoxide`, `oh-my-posh`, `tmux`

## High-signal system introspection commands

```sh
sw_vers
system_profiler SPHardwareDataType -detailLevel mini
uname -a
brew --prefix
brew leaves
```

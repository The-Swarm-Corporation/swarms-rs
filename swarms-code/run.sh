#!/bin/sh
# Build Swarms Code and start chatting with it.
#
#   ./swarms-code/run.sh                     chat about the current directory
#   ./swarms-code/run.sh -C ~/code/project   chat about another project
#   ./swarms-code/run.sh -p "explain main.rs"  one answer, no chat
#   ./swarms-code/run.sh --help              every option
#
# Needs Rust (https://rustup.rs) and an API key: ANTHROPIC_API_KEY for Claude models, or
# OPENROUTER_API_KEY for OpenRouter's models. Keys can live in this repository's .env file.
# In the chat, /model picks a model and /effort how hard it thinks; both are remembered.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found. Install Rust from https://rustup.rs, then run this again." >&2
    exit 1
fi

cargo build --release --manifest-path "$ROOT/Cargo.toml" -p swarms-code

# Take API keys from the repository's .env when they aren't already exported, so the chat
# works from any directory. Only well-formed *_API_KEY entries are read.
if [ -f "$ROOT/.env" ]; then
    while IFS='=' read -r name value || [ -n "$name" ]; do
        name=${name#export }
        case "$name" in
            *[!A-Za-z0-9_]* | "") continue ;;
            *_API_KEY) ;;
            *) continue ;;
        esac
        if [ -z "$(printenv "$name" 2>/dev/null || true)" ]; then
            value=${value%\"}
            value=${value#\"}
            value=${value%\'}
            value=${value#\'}
            export "$name=$value"
        fi
    done <"$ROOT/.env"
fi

exec "$ROOT/target/release/swarms-code" "$@"

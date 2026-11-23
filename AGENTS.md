# AGENTS.md

Rules for coding agents working in this repository.

## Comments

- Multi-line comments are banned. Every comment must fit on a single line.
- This covers `/* ... */` blocks, runs of consecutive `//` lines, and multi-line `///` or `//!` doc comments.
- Never put conversation details in comments. Prompts, chat history, user instructions, reasoning, and notes about the agent session must not appear anywhere in the code.
- A comment states what the code does or why. If it needs more than one line, rename things or restructure the code instead.

## Checks

Run these before opening a PR:

```
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

# AGENTS.md

Rules for coding agents working in this repository.

## Comments

- Multi-line comments are banned. Every comment must fit on a single line.
- This covers `/* ... */` blocks, runs of consecutive `//` lines, and multi-line `///` or `//!` doc comments.
- Never put conversation details in comments. Prompts, chat history, user instructions, reasoning, and notes about the agent session must not appear anywhere in the code.
- A comment states what the code does or why. If it needs more than one line, rename things or restructure the code instead.

## Scope

- One issue per PR. Change only what the issue asks for.
- Do not add dependencies without asking first. `deny.toml` checks licenses and advisories, and every new dependency adds compile time.
- Delete dead code. Never leave code commented out.
- PRs that remove code, such as whole methods, must be net deletive: they delete more lines than they add.

## Tests

- Add a test only when it checks behavior the PR changes.
- A bug fix gets a test that fails without the fix.
- Docs-only and comment-only PRs get no tests.
- Put new tests in an existing file in `swarms-rs/tests/` when one covers the area. Create a new file only when none does.

## Pull requests

- PR titles follow the format `[TYPE][Component::item][Summary]`, e.g. `[BUG][AgentConversation::push][Stop dropping the save task handle and its errors]`.
- `TYPE` matches the issue: `BUG`, `FEATURE`, `IMPROVEMENT`, or `CHORE`.

## Checks

Run these before opening a PR:

```
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

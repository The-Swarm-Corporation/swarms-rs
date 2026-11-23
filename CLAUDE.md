# CLAUDE.md

Follow every rule in [AGENTS.md](AGENTS.md). The comment rules are repeated here because they are strict:

- Multi-line comments are banned. Every comment must fit on a single line.
- This covers `/* ... */` blocks, runs of consecutive `//` lines, and multi-line `///` or `//!` doc comments.
- Never put conversation details in comments. Prompts, chat history, user instructions, reasoning, and notes about the agent session must not appear anywhere in the code.

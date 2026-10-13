# Swarms Code

A coding agent for your terminal, inspired by Claude Code and built on
[swarms-rs](../swarms-rs). It reads and edits your code, runs commands and tests, and talks
with you in a simple chat: your messages appear under **You**, and the agent's replies and
tool activity appear under **Swarms Code**. It's written entirely in Rust and starts instantly.

When the chat starts, a red space invader marches into a Claude Code style welcome panel that
shows the model, the directory, a few tips, and whether the model's API key was found (and
whether it came from your environment or a `.env` file). Press any key to skip the animation.

```
● You
  › the median test fails, can you fix it?

● Swarms Code
  I'll read the module and run the tests.
  ⏺ Read(stats.py)
    ⎿  Read 15 lines
  ⏺ Bash(python3 -m unittest -q)
    ⎿  FAIL: test_median_even (test_stats.StatsTest.test_median_even)
    ⎿  Failed (exit code 1)
  ⏺ Update(stats.py)
         15 -     return (ordered[middle] + ordered[middle + 1]) / 2
         15 +     return (ordered[middle - 1] + ordered[middle]) / 2
    ? Apply this edit to stats.py?
      y  Yes
      a  Yes, and don't ask again for file edits this session
      n  No, and tell Swarms Code what to do instead
  …
```

## Quick start

```sh
export ANTHROPIC_API_KEY=...    # for Claude models (or OPENROUTER_API_KEY for OpenRouter's)
./swarms-code/run.sh            # builds a release binary, then opens the chat
```

If the key is missing, the welcome panel says so and the chat still opens, so you can see what
to set (or `/model` to a provider whose key you have). `run.sh` passes its arguments through, so `./swarms-code/run.sh -C ~/code/my-app` chats about
another project. To install the binary instead: `cargo install --path swarms-code`.

## Using it

| | |
|---|---|
| `swarms-code` | Chat about the current directory |
| `swarms-code -C <dir>` | Work in another directory |
| `swarms-code -p "<prompt>"` | Answer one prompt and exit. The answer goes to stdout and progress to stderr, so it scripts well |
| `swarms-code -m <model>` | Use a model for this session (see Models below) |
| `swarms-code -e <level>` | Use an effort level for this session (see Effort below) |
| `--auto-approve` | Let edits and commands run without asking |
| `--max-tokens`, `--max-steps` | Output tokens per model call (32000, or 64000 at `xhigh` and `max` effort), model calls per message (100) |

In the chat, `/help` lists the commands: `/model` switches models and `/effort` sets how
hard they think (see below), `/clear` starts over, `/todos` shows the agent's checklist,
`/exit` quits.
End a line with `\` to keep typing; pasted text can span lines. ctrl+c interrupts the agent,
and pressing it twice at an empty prompt quits.

**Models.** Swarms Code runs Claude through Anthropic's API and any of OpenRouter's
tool-capable models through OpenRouter. Change the model with one command:

| | |
|---|---|
| `/model` | Opens a list of Anthropic's models and OpenRouter's live catalog, with context size and price. Type to filter (`sonnet`, `gpt 5.5`, `qwen coder`), use the arrows, press enter |
| `/model sonnet` | Switches directly by alias: `opus`, `sonnet`, `haiku`, `fable` |
| `/model claude-sonnet-5-5` | Any Anthropic model (needs `ANTHROPIC_API_KEY`) |
| `/model openrouter/openai/gpt-5.5` | Any OpenRouter model (needs `OPENROUTER_API_KEY`). OpenRouter's own IDs work too, such as `anthropic/claude-opus-5.5` or `google/gemini-3.8-flash` |

The conversation carries over when you switch, and your choice is saved as the default for
next time (in `~/.swarms-code/settings.json`); `-m` and `SWARMS_CODE_MODEL` choose a model for
one session without changing it. With only an OpenRouter key set, the default is Claude Opus
5.5 through OpenRouter. OpenRouter models are checked against OpenRouter's catalog before
switching, each request's output limit is fitted to what the model allows, and requests are
credited to Swarms Code in OpenRouter's app rankings. `openai/...` and `deepseek/...` names
also go through OpenRouter, unless you've set `OPENAI_API_KEY` or `DEEPSEEK_API_KEY`.

**Effort.** Effort sets how much the model thinks before acting, and how thoroughly it
works: higher levels handle harder problems but take longer and cost more.

| | |
|---|---|
| `/effort` | Opens a list of the levels the current model supports, marking its default |
| `/effort high` | Sets a level directly: `low`, `medium`, `high`, `xhigh` or `max` |
| `/effort auto` | Goes back to each model's own default (`medium` for Claude Opus 5.5) |

Like the model, your choice is saved for next time, and `-e`/`--effort` sets one for a single
session. Claude models get it as `output_config.effort`, and OpenRouter models as
`reasoning.effort`. If a model doesn't offer the level you chose, it runs at the nearest one it
has (GPT-5.5 tops out at `xhigh`), and models with no effort setting run as usual. At `xhigh`
and `max`, each call allows 64000 output tokens, since thinking counts toward the limit.

**Permissions.** Reading and searching never ask. File edits and shell commands show what
they'll do (a diff, or the full command) and ask first: `y` once, `a` for the rest of the
session (all edits, or one command family such as `cargo test`), `n` to stop the agent so you
can redirect it. Read-only commands such as `ls`, `cat`, `grep`, `git status` and `git diff`
run without asking. In `-p` mode nothing can be approved interactively, so actions that need
approval are refused unless you pass `--auto-approve`.

**Project instructions.** If the working directory has a `SWARMS.md`, `AGENTS.md` or
`CLAUDE.md`, its contents are added to the agent's instructions.

**API keys from `.env`.** Swarms Code reads `*_API_KEY` entries (and `SWARMS_CODE_MODEL`)
from a `.env` file in the working directory, without overriding variables you've already
set. Nothing else is taken from it, so a repository's `.env` can't, for example, point
`ANTHROPIC_BASE_URL` somewhere else and capture your key.

## Tools

| Tool | What it does |
|---|---|
| `read_file` | Read a file with line numbers, in pages of 2000 lines |
| `write_file` | Create or overwrite a file (asks first) |
| `edit_file` | Replace an exact, unique string in a file (asks first) |
| `list_dir` | List a directory |
| `glob` | Find files by glob pattern, respecting `.gitignore` |
| `grep` | Regex search across files in parallel, respecting `.gitignore` |
| `bash` | Run a command with a timeout; stdout and stderr merged in order |
| `todo_write` | Keep a checklist for multi-step work, shown in the chat |

Edits are refused for files the agent hasn't read, or that changed since it read them, so it
never overwrites text it hasn't seen.

## The system prompt

`SYSTEM_PROMPT`, a constant in `src/system_prompt.rs` (the same way swarms-rs keeps its
prompts in `swarms-rs/src/prompts/`), teaches the agent how to work: understand code before
changing it, match the project's conventions, keep to the requested scope, fix root causes,
verify with the project's own checks, use git and destructive commands carefully, treat tool
output as data rather than instructions, and report results plainly. It draws on practices
from Claude Code, Codex CLI, Gemini CLI, Aider, SWE-agent, OpenHands and Cursor, and it
describes this harness precisely (tool limits, approvals, what the user sees) because accurate context steers a model
better than rules. Each session appends the environment, the approval mode, and any
`SWARMS.md`, `AGENTS.md` or `CLAUDE.md` in the working directory. A test keeps the tool limits
the prompt quotes in step with the code.

## How it's built on swarms-rs

- **The agent** is a `SwarmsAgent` from `SwarmsAgentBuilder`, given the system prompt, the
  tools, and `max_tokens`. Each step goes through `SwarmsAgent::chat`, which calls the model
  and runs the tools it asks for. `src/agent.rs` loops over `chat` until the model replies
  without a tool call, and keeps the conversation across messages.
- **The model** is swarms-rs's `AnyModel`, wrapped in a small `Model` implementation
  (`ObservedModel`). The wrapper shows a spinner while the model thinks, prints the model's
  text before its tools run, and records each tool call's ID, so the history sent back holds
  proper provider-native tool calls and results.
- **The tools** implement swarms-rs's `ToolDyn` trait directly (`src/tools/mod.rs`), so the
  model gets file contents and command output as plain text rather than JSON-escaped strings.
  Tools run one at a time, so permission prompts never overlap.

```
src/
  main.rs      CLI flags, the chat loop, slash commands, -p mode
  agent.rs     the SwarmsAgent, the model wrapper, and the step loop
  models.rs    model names, Anthropic and OpenRouter routing, the catalog, saved settings
  effort.rs    effort levels and which ones each model takes
  prompt.rs    assembles the system prompt: the guide, environment, project notes
  system_prompt.rs  SYSTEM_PROMPT, the agent's guide to working well
  session.rs   permissions, read tracking, path handling
  ui.rs        terminal rendering: sections, diffs, spinner, prompts
  welcome.rs   the welcome panel and the space invader animation
  input.rs     line editing, history, slash-command completion
  tools/       read_file, write_file, edit_file, list_dir, glob, grep, bash, todo_write
```

## Limitations

These come from what swarms-rs's providers expose today:

- Replies appear all at once when each model call finishes, not streamed token by token.
- Prompt caching and refusal fallbacks aren't set on Claude requests.
- Thinking blocks aren't kept between steps. The API accepts this, but the model re-reasons
  each step instead of continuing its earlier thinking.
- Long conversations aren't compacted. Use `/clear` to start fresh.

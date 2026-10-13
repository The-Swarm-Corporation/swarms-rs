//! The bash tool: run a shell command in the working directory.

use std::{fmt::Write as _, process::Stdio, sync::Arc, time::Duration};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;

use super::{CodingTool, DENIED, truncate_middle};
use crate::session::{Approval, Session};

pub(crate) const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub(crate) const MAX_TIMEOUT_SECS: u64 = 600;
pub(crate) const MAX_OUTPUT_CHARS: usize = 30_000;

pub struct Bash(pub Arc<Session>);

#[derive(Deserialize)]
pub struct BashArgs {
    command: String,
    timeout: Option<u64>,
}

impl CodingTool for Bash {
    type Args = BashArgs;

    const NAME: &'static str = "bash";
    const DESCRIPTION: &'static str = "Run a shell command with bash in the working directory and \
        return its combined stdout and stderr, plus the exit code when it fails. Each call starts \
        a fresh shell, so `cd` and exported variables don't carry over; chain dependent steps \
        with `&&`. Use it to build, test, run programs and use git; prefer read_file, grep, glob \
        and list_dir for reading and searching. Commands time out after `timeout` seconds \
        (default 120, max 600). Don't start interactive programs or servers that never exit. \
        Commands that may change anything ask the user for approval first.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The command to run" },
                "timeout": { "type": "integer", "minimum": 1, "maximum": MAX_TIMEOUT_SECS, "description": "Seconds before the command is killed (default 120)" }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: BashArgs) -> Result<String, String> {
        let session = &self.0;
        let command = args.command.trim();
        session.ui.tool_start("Bash", command);
        if command.is_empty() {
            return Err("The command is empty".to_owned());
        }

        if !is_read_only(command) {
            // The header shows one line; the user must see all of what they're approving.
            session.ui.full_command(command);
            let prefix = command_prefix(command);
            let approval = Approval::Command {
                prefix: prefix.as_deref(),
            };
            if !session.approve(approval, "Run this command?").await {
                return Ok(DENIED.to_owned());
            }
        }

        let secs = args
            .timeout
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .clamp(1, MAX_TIMEOUT_SECS);
        let child = shell(command)
            .current_dir(&session.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Stop the command if the turn is interrupted or it times out.
            .kill_on_drop(true)
            // Commands run without a terminal; keep git and pagers from waiting for input.
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_PAGER", "cat")
            .env("PAGER", "cat")
            .spawn()
            .map_err(|e| format!("Couldn't start the shell: {e}"))?;

        let spinner = session.ui.spinner("Running");
        let output =
            tokio::time::timeout(Duration::from_secs(secs), child.wait_with_output()).await;
        drop(spinner);
        let output = match output {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return Err(format!("The command failed to run: {e}")),
            Err(_) => {
                return Err(format!(
                    "The command timed out after {secs}s and was stopped"
                ));
            },
        };

        let mut text = String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(stderr.trim_end());
        }
        session.ui.tool_output(&text);

        let mut result = if text.is_empty() {
            "(no output)".to_owned()
        } else {
            truncate_middle(&text, MAX_OUTPUT_CHARS)
        };
        if !output.status.success() {
            let status = output.status.code().map_or_else(
                || "killed by a signal".to_owned(),
                |code| format!("exit code {code}"),
            );
            session.ui.tool_error(&format!("Failed ({status})"));
            let _ = write!(result, "\n\n[{status}]");
        }
        Ok(result)
    }
}

#[cfg(unix)]
fn shell(command: &str) -> Command {
    let program = if std::path::Path::new("/bin/bash").exists() {
        "/bin/bash"
    } else {
        "/bin/sh"
    };
    let mut cmd = Command::new(program);
    // Merge stderr into stdout so the model sees output in the order it was printed.
    cmd.arg("-c").arg(format!("exec 2>&1\n{command}"));
    cmd
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(command);
    cmd
}

/// Chaining, redirection, pipes and substitution: anything that makes a command more than
/// one simple program invocation.
fn has_shell_operators(command: &str) -> bool {
    command.contains(['&', '|', ';', '>', '<', '`', '\n']) || command.contains("$(")
}

/// What "don't ask again" covers for a command: the program, plus its subcommand for tools
/// like git and cargo (`cargo test`). `None` for compound commands, which always ask.
pub fn command_prefix(command: &str) -> Option<String> {
    const WITH_SUBCOMMANDS: [&str; 22] = [
        "bun", "cargo", "deno", "docker", "gh", "git", "go", "gradle", "just", "kubectl", "make",
        "mvn", "node", "npm", "npx", "pip", "pip3", "pnpm", "python", "python3", "uv", "yarn",
    ];
    if has_shell_operators(command) {
        return None;
    }
    let mut words = command.split_whitespace();
    let program = words.next()?;
    match words.next() {
        Some(sub) if WITH_SUBCOMMANDS.contains(&program) && !sub.starts_with('-') => {
            Some(format!("{program} {sub}"))
        },
        _ => Some(program.to_owned()),
    }
}

/// Commands that only read, which run without asking.
fn is_read_only(command: &str) -> bool {
    if has_shell_operators(command) || command.contains('$') {
        return false;
    }
    let words: Vec<&str> = command.split_whitespace().collect();
    let Some((&program, args)) = words.split_first() else {
        return false;
    };
    let any_arg = |test: &dyn Fn(&str) -> bool| args.iter().any(|arg| test(arg));
    match program {
        "ls" | "pwd" | "cat" | "head" | "wc" | "stat" | "du" | "df" | "which" | "echo" | "date"
        | "whoami" | "uname" | "grep" => true,
        "tail" => {
            !any_arg(&|arg| arg == "--follow" || (arg.starts_with('-') && arg.contains(['f', 'F'])))
        },
        "rg" => !any_arg(&|arg| arg.starts_with("--pre")),
        "find" => !any_arg(&|arg| {
            matches!(
                arg,
                "-exec" | "-execdir" | "-ok" | "-okdir" | "-delete" | "-fls"
            ) || arg.starts_with("-fprint")
        }),
        "git" => {
            matches!(args.first(), Some(&("status" | "diff" | "log" | "show")))
                && !any_arg(&|arg| arg.starts_with("--output") || arg == "--ext-diff")
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_commands_skip_approval() {
        for command in [
            "ls -la src",
            "git status",
            "git diff HEAD~1",
            "grep -rn foo .",
            "find . -name '*.rs'",
        ] {
            assert!(is_read_only(command), "{command}");
        }
        for command in [
            "rm -rf build",
            "git push",
            "git diff --output=patch.txt",
            "find . -delete",
            "ls; rm -rf /",
            "cat a > b",
            "echo $(whoami)",
            "tail -f log.txt",
            "rg --pre ./script foo",
        ] {
            assert!(!is_read_only(command), "{command}");
        }
    }

    #[test]
    fn prefixes_cover_program_and_subcommand() {
        assert_eq!(
            command_prefix("cargo test --lib parser").as_deref(),
            Some("cargo test")
        );
        assert_eq!(command_prefix("npm run build").as_deref(), Some("npm run"));
        assert_eq!(
            command_prefix("pytest -q tests/").as_deref(),
            Some("pytest")
        );
        assert_eq!(command_prefix("git --no-pager log").as_deref(), Some("git"));
        assert_eq!(command_prefix("cargo test && rm -rf /"), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shell_merges_output_streams_in_order() {
        let output = shell("echo out; echo err >&2; echo done")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "out\nerr\ndone\n");
        assert!(output.stderr.is_empty());
    }
}

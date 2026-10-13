//! The agent's system prompt: how to work, the environment, and any project instructions.

use std::{fmt::Write as _, fs, path::Path};

use crate::system_prompt::SYSTEM_PROMPT;

/// Project instruction files read from the working directory, in this order.
const INSTRUCTION_FILES: [&str; 3] = ["SWARMS.md", "AGENTS.md", "CLAUDE.md"];
const MAX_INSTRUCTION_CHARS: usize = 40_000;

/// Whether tool calls that need approval will be put to the user.
#[derive(Clone, Copy)]
pub enum Approvals {
    /// The user is asked about each edit and command.
    Ask,
    /// --auto-approve: everything runs without asking.
    Auto,
    /// Print mode without --auto-approve: nobody can approve, so those calls are refused.
    Refused,
}

pub fn build(cwd: &Path, model: &str, approvals: Approvals) -> String {
    let mut prompt = String::from(SYSTEM_PROMPT.trim());

    let _ = write!(
        prompt,
        "\n\n# Environment\n- Working directory: {}\n- Platform: {} ({})\n- Today's date: {}\n- Model: {model}\n",
        cwd.display(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        chrono::Local::now().format("%Y-%m-%d"),
    );
    match git_status(cwd) {
        Git::Branch(branch) => {
            let _ = writeln!(prompt, "- Git repository: yes, on branch {branch}");
        },
        Git::NoBranch => prompt.push_str("- Git repository: yes\n"),
        Git::None => prompt.push_str("- Git repository: no\n"),
    }
    prompt.push_str(match approvals {
        Approvals::Ask => {
            "- Approval mode: the user is asked before each file edit and each command that \
             might change anything.\n"
        },
        Approvals::Auto => {
            "- Approval mode: auto-approve is on, so edits and commands run without the user \
             reviewing them first. Take extra care with anything destructive.\n"
        },
        Approvals::Refused => {
            "- Approval mode: this is a one-shot run with nobody to approve actions, so file \
             edits and commands that might change anything will be refused. Your final message \
             is printed as the result.\n"
        },
    });

    for name in INSTRUCTION_FILES {
        let Ok(text) = fs::read_to_string(cwd.join(name)) else {
            continue;
        };
        let text: String = text.chars().take(MAX_INSTRUCTION_CHARS).collect();
        let _ = write!(
            prompt,
            "\n# Project instructions ({name})\nThe project's own instructions. Follow them \
             unless the user says otherwise.\n\n{}\n",
            text.trim()
        );
    }
    prompt
}

/// The project instructions file in `cwd`, if there is one.
pub fn notes_file(cwd: &Path) -> Option<&'static str> {
    INSTRUCTION_FILES
        .into_iter()
        .find(|name| cwd.join(name).is_file())
}

enum Git {
    None,
    Branch(String),
    /// A repository whose branch can't be read, e.g. a detached HEAD or a worktree.
    NoBranch,
}

fn git_status(cwd: &Path) -> Git {
    let Some(git) = cwd
        .ancestors()
        .map(|dir| dir.join(".git"))
        .find(|git| git.exists())
    else {
        return Git::None;
    };
    fs::read_to_string(git.join("HEAD"))
        .ok()
        .and_then(|head| {
            head.trim()
                .strip_prefix("ref: refs/heads/")
                .map(str::to_owned)
        })
        .map_or(Git::NoBranch, Git::Branch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{
        CodingTool, bash,
        fs::{self as file_tools, EditFile, ListDir, ReadFile, WriteFile},
        search::{self, Glob, Grep},
        todo::TodoWrite,
    };

    #[test]
    fn guide_names_every_tool_and_its_real_limits() {
        for name in [
            ReadFile::NAME,
            WriteFile::NAME,
            EditFile::NAME,
            ListDir::NAME,
            Glob::NAME,
            Grep::NAME,
            bash::Bash::NAME,
            TodoWrite::NAME,
        ] {
            assert!(
                SYSTEM_PROMPT.contains(name),
                "the guide never mentions {name}"
            );
        }
        // The guide quotes these limits; keep it in step when they change.
        let limits = [
            format!("up to {} at a time", file_tools::DEFAULT_READ_LINES),
            format!("at most {} matching lines", search::MAX_GREP_LINES),
            format!("at most {} paths", search::MAX_GLOB_RESULTS),
            format!(
                "after {} seconds by default (up to {}",
                bash::DEFAULT_TIMEOUT_SECS,
                bash::MAX_TIMEOUT_SECS
            ),
        ];
        for limit in limits {
            assert!(
                SYSTEM_PROMPT.contains(&limit),
                "the guide doesn't say {limit:?}"
            );
        }
        assert_eq!(
            bash::MAX_OUTPUT_CHARS,
            30_000,
            "the guide says about 30,000 characters"
        );
    }

    #[test]
    fn prompt_includes_environment_mode_and_project_notes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("SWARMS.md"),
            "Run `make check` before finishing.",
        )
        .unwrap();
        let prompt = build(dir.path(), "claude-opus-5-5", Approvals::Refused);
        assert!(prompt.starts_with("You are Swarms Code"));
        assert!(prompt.contains("- Model: claude-opus-5-5"));
        assert!(prompt.contains("nobody to approve actions"));
        assert!(prompt.contains("# Project instructions (SWARMS.md)"));
        assert!(prompt.contains("Run `make check` before finishing."));
        assert!(build(dir.path(), "m", Approvals::Auto).contains("auto-approve is on"));
    }
}

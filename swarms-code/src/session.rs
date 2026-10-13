//! State shared by the agent's tools for one Swarms Code session: the working directory,
//! permission decisions, which files the agent has read, and its todo list.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use crate::{
    lock,
    prompt::Approvals,
    tools::todo::Todo,
    ui::{Answer, Ui},
};

/// What a tool needs the user's go-ahead for.
pub enum Approval<'a> {
    /// Writing or editing a file.
    Edit,
    /// Running a shell command. `prefix` (e.g. `cargo test`) is what "don't ask again" covers;
    /// `None` for commands that chain or redirect, which are always asked about.
    Command { prefix: Option<&'a str> },
}

pub struct Session {
    pub cwd: PathBuf,
    pub ui: Ui,
    auto_approve: bool,
    /// False when there's nobody to ask (print mode): anything needing approval is denied.
    interactive: bool,
    edits_allowed: AtomicBool,
    allowed_commands: Mutex<HashSet<String>>,
    /// Files the agent has read, with their modification time at that point.
    reads: Mutex<HashMap<PathBuf, Option<SystemTime>>>,
    todos: Mutex<Vec<Todo>>,
    /// Set when the user denies a tool call; the turn stops so they can redirect the agent.
    halted: AtomicBool,
}

impl Session {
    pub fn new(cwd: PathBuf, ui: Ui, auto_approve: bool, interactive: bool) -> Self {
        Self {
            cwd,
            ui,
            auto_approve,
            interactive,
            edits_allowed: AtomicBool::new(false),
            allowed_commands: Mutex::default(),
            reads: Mutex::default(),
            todos: Mutex::default(),
            halted: AtomicBool::new(false),
        }
    }

    pub fn auto_approve(&self) -> bool {
        self.auto_approve
    }

    pub fn approvals(&self) -> Approvals {
        match (self.auto_approve, self.interactive) {
            (true, _) => Approvals::Auto,
            (false, true) => Approvals::Ask,
            (false, false) => Approvals::Refused,
        }
    }

    /// Ask the user whether a tool may go ahead, unless they already allowed it.
    pub async fn approve(&self, approval: Approval<'_>, question: &str) -> bool {
        if self.auto_approve || self.is_allowed(&approval) {
            return true;
        }
        if !self.interactive {
            self.ui
                .tool_error("Needs approval; rerun with --auto-approve to allow it");
            self.halt();
            return false;
        }
        let always = match &approval {
            Approval::Edit => Some("file edits".to_owned()),
            Approval::Command {
                prefix: Some(prefix),
            } => Some(format!("`{prefix}` commands")),
            Approval::Command { prefix: None } => None,
        };
        match self.ui.ask_permission(question, always.as_deref()).await {
            Answer::Yes => true,
            Answer::Always => {
                match approval {
                    Approval::Edit => self.edits_allowed.store(true, Ordering::Relaxed),
                    Approval::Command { prefix } => {
                        if let Some(prefix) = prefix {
                            lock(&self.allowed_commands).insert(prefix.to_owned());
                        }
                    },
                }
                true
            },
            Answer::No => {
                self.halt();
                false
            },
        }
    }

    fn is_allowed(&self, approval: &Approval<'_>) -> bool {
        match approval {
            Approval::Edit => self.edits_allowed.load(Ordering::Relaxed),
            Approval::Command {
                prefix: Some(prefix),
            } => lock(&self.allowed_commands).contains(*prefix),
            Approval::Command { prefix: None } => false,
        }
    }

    pub fn halt(&self) {
        self.halted.store(true, Ordering::Relaxed);
    }

    pub fn halted(&self) -> bool {
        self.halted.load(Ordering::Relaxed)
    }

    pub fn begin_turn(&self) {
        self.halted.store(false, Ordering::Relaxed);
    }

    /// Forget the conversation-specific state (for `/clear`). Permissions granted for the
    /// session stay granted.
    pub fn reset(&self) {
        lock(&self.reads).clear();
        lock(&self.todos).clear();
    }

    /// Resolve a path the model gave (relative, absolute, or `~/...`) against the working
    /// directory.
    pub fn resolve(&self, raw: &str) -> PathBuf {
        let raw = raw.trim();
        let expanded = match (raw.strip_prefix("~/"), home_dir()) {
            (Some(rest), Some(home)) => home.join(rest),
            _ if raw == "~" => home_dir().unwrap_or_else(|| PathBuf::from(raw)),
            _ => PathBuf::from(raw),
        };
        normalize(&self.cwd.join(expanded))
    }

    /// A path as shown to the user and the model: relative to the working directory when
    /// it's inside it.
    pub fn display(&self, path: &Path) -> String {
        if let Ok(relative) = path.strip_prefix(&self.cwd) {
            let shown = relative.display().to_string();
            return if shown.is_empty() {
                ".".to_owned()
            } else {
                shown
            };
        }
        tilde(path)
    }

    pub fn mark_read(&self, path: &Path) {
        lock(&self.reads).insert(path.to_path_buf(), modified(path));
    }

    /// Edits and overwrites need a current read of the file, so the agent never changes
    /// text it hasn't seen (or that changed under it).
    pub fn ensure_fresh(&self, path: &Path, shown: &str) -> Result<(), String> {
        match lock(&self.reads).get(path) {
            None => Err(format!("Read {shown} with read_file before changing it.")),
            Some(seen) if *seen != modified(path) => Err(format!(
                "{shown} has changed since you last read it. Read it again before changing it."
            )),
            Some(_) => Ok(()),
        }
    }

    pub fn todos(&self) -> Vec<Todo> {
        lock(&self.todos).clone()
    }

    pub fn set_todos(&self, todos: Vec<Todo>) {
        *lock(&self.todos) = todos;
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

/// Resolve `.` and `..` without touching the file system (the path may not exist yet).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir => {
                out.pop();
            },
            other => out.push(other),
        }
    }
    out
}

/// A path with the home directory shortened to `~`.
pub fn tilde(path: &Path) -> String {
    if let Some(home) = home_dir()
        && let Ok(relative) = path.strip_prefix(&home)
    {
        return format!("~/{}", relative.display());
    }
    path.display().to_string()
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(cwd: &str) -> Session {
        Session::new(PathBuf::from(cwd), Ui::new(true), false, false)
    }

    #[test]
    fn resolves_paths_against_the_working_directory() {
        let s = session("/work/project");
        assert_eq!(
            s.resolve("src/main.rs"),
            PathBuf::from("/work/project/src/main.rs")
        );
        assert_eq!(
            s.resolve("./src/../Cargo.toml"),
            PathBuf::from("/work/project/Cargo.toml")
        );
        assert_eq!(s.resolve("/etc/hosts"), PathBuf::from("/etc/hosts"));
        assert_eq!(
            s.display(Path::new("/work/project/src/lib.rs")),
            "src/lib.rs"
        );
        assert_eq!(s.display(Path::new("/work/project")), ".");
    }

    #[test]
    fn edits_require_a_current_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "one").unwrap();
        let s = session(dir.path().to_str().unwrap());

        assert!(s.ensure_fresh(&file, "a.txt").is_err());
        s.mark_read(&file);
        assert!(s.ensure_fresh(&file, "a.txt").is_ok());

        // A change made outside the agent (a formatter, the user) invalidates the read.
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(s.ensure_fresh(&file, "a.txt").is_err());
    }
}

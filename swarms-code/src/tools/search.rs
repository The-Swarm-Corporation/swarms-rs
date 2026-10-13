//! Search tools: glob (find files by path) and grep (search file contents). Both walk the tree
//! with ripgrep's `ignore` crate, so .gitignore rules apply, and grep searches in parallel.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use globset::{GlobBuilder, GlobMatcher};
use ignore::{WalkBuilder, WalkState};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{CodingTool, looks_binary};
use crate::{lock, session::Session};

pub(crate) const MAX_GLOB_RESULTS: usize = 500;
pub(crate) const MAX_GREP_LINES: usize = 200;
const MAX_GREP_LINE_CHARS: usize = 300;
const MAX_SEARCH_FILE_BYTES: u64 = 10 * 1024 * 1024;

fn walker(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git");
    builder
}

fn compile_glob(pattern: &str) -> Result<GlobMatcher, String> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|e| format!("Invalid glob pattern: {e}"))
}

// ------------------------------------------------------------------------ glob

pub struct Glob(pub Arc<Session>);

#[derive(Deserialize)]
pub struct GlobArgs {
    pattern: String,
    path: Option<String>,
}

impl CodingTool for Glob {
    type Args = GlobArgs;

    const NAME: &'static str = "glob";
    const DESCRIPTION: &'static str = "Find files whose path matches a glob pattern, relative to \
        `path` (default: the working directory). `*` matches within one directory and `**` \
        across any number of directories, e.g. `**/*.rs` or `src/**/test_*.py`. Respects \
        .gitignore. Returns up to 500 paths, sorted.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern, e.g. **/*.rs" },
                "path": { "type": "string", "description": "Directory to search from (default: the working directory)" }
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: GlobArgs) -> Result<String, String> {
        let session = &self.0;
        let root = session.resolve(args.path.as_deref().unwrap_or("."));
        let label = match &args.path {
            Some(_) => format!("{} in {}", args.pattern, session.display(&root)),
            None => args.pattern.clone(),
        };
        session.ui.tool_start("Glob", &label);

        let matcher = compile_glob(&args.pattern)?;
        let walk_root = root.clone();
        let mut found = tokio::task::spawn_blocking(move || find_files(&walk_root, &matcher))
            .await
            .map_err(|e| e.to_string())?;
        found.sort();

        let total = found.len();
        session.ui.tool_done(&format!(
            "Found {total} {}",
            if total == 1 { "file" } else { "files" }
        ));
        if total == 0 {
            return Ok(format!("No files match {}.", args.pattern));
        }
        let mut out: Vec<String> = found
            .iter()
            .take(MAX_GLOB_RESULTS)
            .map(|path| session.display(path))
            .collect();
        if total > MAX_GLOB_RESULTS {
            out.push(format!(
                "… and {} more. Use a more specific pattern.",
                total - MAX_GLOB_RESULTS
            ));
        }
        Ok(out.join("\n"))
    }
}

fn find_files(root: &Path, matcher: &GlobMatcher) -> Vec<PathBuf> {
    walker(root)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter(|entry| {
            entry
                .path()
                .strip_prefix(root)
                .is_ok_and(|relative| matcher.is_match(relative))
        })
        .map(ignore::DirEntry::into_path)
        .collect()
}

// ------------------------------------------------------------------------ grep

pub struct Grep(pub Arc<Session>);

#[derive(Deserialize)]
pub struct GrepArgs {
    pattern: String,
    path: Option<String>,
    glob: Option<String>,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    files_with_matches: bool,
}

impl CodingTool for Grep {
    type Args = GrepArgs;

    const NAME: &'static str = "grep";
    const DESCRIPTION: &'static str = "Search file contents for a regular expression (Rust regex \
        syntax), recursively from `path` (default: the working directory; a single file also \
        works). Respects .gitignore and skips binary files. Returns matching lines as \
        `path:line: text`, at most 200; set `files_with_matches` to list matching files \
        instead. `glob` limits which files are searched, e.g. `*.rs` or `src/**/*.ts`.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regular expression to search for" },
                "path": { "type": "string", "description": "File or directory to search (default: the working directory)" },
                "glob": { "type": "string", "description": "Only search files matching this glob. Without a `/` it matches file names (`*.rs`); with one it matches paths relative to `path`." },
                "case_insensitive": { "type": "boolean", "description": "Ignore case (default false)" },
                "files_with_matches": { "type": "boolean", "description": "List matching files with match counts instead of matching lines (default false)" }
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: GrepArgs) -> Result<String, String> {
        let session = &self.0;
        let root = session.resolve(args.path.as_deref().unwrap_or("."));
        let label = match &args.path {
            Some(_) => format!("{} in {}", args.pattern, session.display(&root)),
            None => args.pattern.clone(),
        };
        session.ui.tool_start("Grep", &label);

        let regex = RegexBuilder::new(&args.pattern)
            .case_insensitive(args.case_insensitive)
            .build()
            .map_err(|e| format!("Invalid regex: {e}"))?;
        let filter = args
            .glob
            .as_deref()
            .map(|glob| compile_glob(glob).map(|matcher| (matcher, glob.contains('/'))))
            .transpose()?;
        let search_root = root.clone();
        let hits =
            tokio::task::spawn_blocking(move || search(&search_root, &regex, filter.as_ref()))
                .await
                .map_err(|e| e.to_string())?;

        let total: usize = hits.iter().map(|file| file.count).sum();
        let files = hits.len();
        session.ui.tool_done(&format!(
            "Found {total} {} in {files} {}",
            if total == 1 { "match" } else { "matches" },
            if files == 1 { "file" } else { "files" }
        ));
        if hits.is_empty() {
            return Ok(format!("No matches for {}.", args.pattern));
        }

        let mut out = Vec::new();
        if args.files_with_matches {
            for file in &hits {
                out.push(format!("{} ({})", session.display(&file.path), file.count));
            }
        } else {
            'files: for file in &hits {
                for (line, text) in &file.lines {
                    if out.len() == MAX_GREP_LINES {
                        break 'files;
                    }
                    out.push(format!("{}:{line}: {text}", session.display(&file.path)));
                }
            }
            if total > out.len() {
                out.push(format!(
                    "… {} more matching lines not shown. Narrow the search with `path` or `glob`.",
                    total - out.len()
                ));
            }
        }
        Ok(out.join("\n"))
    }
}

struct FileHits {
    path: PathBuf,
    count: usize,
    /// Matching lines (1-based number, text), up to the overall line limit.
    lines: Vec<(usize, String)>,
}

fn search(root: &Path, regex: &Regex, filter: Option<&(GlobMatcher, bool)>) -> Vec<FileHits> {
    let hits = Mutex::new(Vec::new());
    let lines_kept = AtomicUsize::new(0);
    walker(root).build_parallel().run(|| {
        Box::new(|entry| {
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                return WalkState::Continue;
            }
            let path = entry.path();
            if let Some((matcher, match_path)) = filter {
                let candidate = if *match_path {
                    path.strip_prefix(root).unwrap_or(path)
                } else {
                    Path::new(path.file_name().unwrap_or_default())
                };
                if !matcher.is_match(candidate) {
                    return WalkState::Continue;
                }
            }
            if let Some(file) = search_file(path, regex, &lines_kept) {
                lock(&hits).push(file);
            }
            WalkState::Continue
        })
    });
    let mut hits = hits
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    hits
}

fn search_file(path: &Path, regex: &Regex, lines_kept: &AtomicUsize) -> Option<FileHits> {
    if fs::metadata(path).ok()?.len() > MAX_SEARCH_FILE_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if looks_binary(&bytes) {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut file = FileHits {
        path: path.to_path_buf(),
        count: 0,
        lines: Vec::new(),
    };
    for (index, line) in text.lines().enumerate() {
        if !regex.is_match(line) {
            continue;
        }
        file.count += 1;
        // Keep line text only while under the global cap; counts stay exact.
        if lines_kept.fetch_add(1, Ordering::Relaxed) < MAX_GREP_LINES {
            let line = line.trim_end();
            let line: String = line.chars().take(MAX_GREP_LINE_CHARS).collect();
            file.lines.push((index + 1, line));
        }
    }
    (file.count > 0).then_some(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join(".gitignore"), "target/\n").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {\n    run();\n}\n").unwrap();
        fs::write(root.join("src/nested/lib.rs"), "pub fn run() {}\n").unwrap();
        fs::write(root.join("target/out.rs"), "fn run() {}\n").unwrap();
        fs::write(root.join("README.md"), "Call run() to start\n").unwrap();
        dir
    }

    #[test]
    fn glob_matches_paths_and_respects_gitignore() {
        let dir = tree();
        let root = dir.path();
        let mut all_rust = find_files(root, &compile_glob("**/*.rs").unwrap());
        all_rust.sort();
        assert_eq!(
            all_rust,
            vec![root.join("src/main.rs"), root.join("src/nested/lib.rs")]
        );
        // `*` stays within one directory.
        assert_eq!(
            find_files(root, &compile_glob("src/*.rs").unwrap()),
            vec![root.join("src/main.rs")]
        );
    }

    #[test]
    fn grep_finds_lines_and_filters_by_glob() {
        let dir = tree();
        let root = dir.path();
        let regex = Regex::new(r"run\(\)").unwrap();

        let hits = search(root, &regex, None);
        let found: Vec<(PathBuf, Vec<usize>)> = hits
            .iter()
            .map(|file| {
                (
                    file.path.clone(),
                    file.lines.iter().map(|(n, _)| *n).collect(),
                )
            })
            .collect();
        assert_eq!(
            found,
            vec![
                (root.join("README.md"), vec![1]),
                (root.join("src/main.rs"), vec![2]),
                (root.join("src/nested/lib.rs"), vec![1]),
            ]
        );

        let only_rust = (compile_glob("*.rs").unwrap(), false);
        assert_eq!(search(root, &regex, Some(&only_rust)).len(), 2);
    }
}

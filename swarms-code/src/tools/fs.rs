//! File tools: `read_file`, `write_file`, `edit_file` and `list_dir`.

use std::{fmt::Write as _, io::ErrorKind, path::Path, sync::Arc};

use serde::Deserialize;
use serde_json::{Value, json};

use super::{CodingTool, DENIED, looks_binary};
use crate::session::{Approval, Session};

pub(crate) const DEFAULT_READ_LINES: usize = 2000;
const MAX_LINE_CHARS: usize = 2000;
const MAX_DIR_ENTRIES: usize = 500;

fn io_error(shown: &str, error: &std::io::Error) -> String {
    match error.kind() {
        ErrorKind::NotFound => format!("{shown} does not exist"),
        ErrorKind::PermissionDenied => format!("Permission denied: {shown}"),
        _ => format!("{shown}: {error}"),
    }
}

async fn read_text(path: &Path, shown: &str) -> Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| io_error(shown, &e))?;
    if looks_binary(&bytes) {
        return Err(format!("{shown} is a binary file"));
    }
    String::from_utf8(bytes).map_err(|_| format!("{shown} is not valid UTF-8 text"))
}

// ------------------------------------------------------------------------ read_file

pub struct ReadFile(pub Arc<Session>);

#[derive(Deserialize)]
pub struct ReadArgs {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

impl CodingTool for ReadFile {
    type Args = ReadArgs;

    const NAME: &'static str = "read_file";
    const DESCRIPTION: &'static str = "Read a text file. Returns its lines prefixed with 1-based \
        line numbers (the number, a tab, then the line). Reads up to 2000 lines starting at \
        `offset`; for longer files, call again with a later offset. Read a file before you edit \
        or overwrite it: edit_file and write_file refuse files you haven't read.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path, relative to the working directory or absolute" },
                "offset": { "type": "integer", "minimum": 1, "description": "1-based line to start reading from (default 1)" },
                "limit": { "type": "integer", "minimum": 1, "description": "Maximum number of lines to read (default 2000)" }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: ReadArgs) -> Result<String, String> {
        let session = &self.0;
        let path = session.resolve(&args.path);
        let shown = session.display(&path);
        session.ui.tool_start("Read", &shown);

        if path.is_dir() {
            return Err(format!(
                "{shown} is a directory; use list_dir to see its contents"
            ));
        }
        let text = read_text(&path, &shown).await?;
        session.mark_read(&path);

        let total = text.lines().count();
        if total == 0 {
            session.ui.tool_done("Empty file");
            return Ok(format!("{shown} is empty."));
        }
        let start = args.offset.unwrap_or(1).max(1);
        if start > total {
            return Err(format!(
                "offset {start} is past the end of {shown} ({total} lines)"
            ));
        }
        let limit = args.limit.unwrap_or(DEFAULT_READ_LINES).max(1);
        let mut out = String::new();
        let mut shown_lines = 0;
        for (index, line) in text.lines().enumerate().skip(start - 1).take(limit) {
            let line: String = line.chars().take(MAX_LINE_CHARS).collect();
            let _ = writeln!(out, "{:>6}\t{line}", index + 1);
            shown_lines += 1;
        }
        let end = start - 1 + shown_lines;
        if end < total {
            let _ = write!(
                out,
                "\n({} more lines. Call read_file with offset {} to continue.)",
                total - end,
                end + 1
            );
            session
                .ui
                .tool_done(&format!("Read lines {start}-{end} of {total}"));
        } else {
            session.ui.tool_done(&format!("Read {shown_lines} lines"));
        }
        Ok(out)
    }
}

// ------------------------------------------------------------------------ write_file

pub struct WriteFile(pub Arc<Session>);

#[derive(Deserialize)]
pub struct WriteArgs {
    path: String,
    content: String,
}

impl CodingTool for WriteFile {
    type Args = WriteArgs;

    const NAME: &'static str = "write_file";
    const DESCRIPTION: &'static str = "Create a file, or replace an existing file's entire \
        contents, with `content`. Missing parent directories are created. To change part of an \
        existing file use edit_file instead. Overwriting a file requires that you read it \
        first. The user is asked to approve the write.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path, relative to the working directory or absolute" },
                "content": { "type": "string", "description": "The complete new contents of the file" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: WriteArgs) -> Result<String, String> {
        let session = &self.0;
        let path = session.resolve(&args.path);
        let shown = session.display(&path);
        session.ui.tool_start("Write", &shown);

        if path.is_dir() {
            return Err(format!("{shown} is a directory"));
        }
        let existing = match tokio::fs::read(&path).await {
            Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(io_error(&shown, &e)),
        };
        if existing.is_some() {
            session.ensure_fresh(&path, &shown)?;
        }
        if existing.as_deref() == Some(args.content.as_str()) {
            session.ui.tool_done("No changes");
            return Ok(format!(
                "{shown} already has exactly this content; nothing was written."
            ));
        }

        let (added, removed) = session
            .ui
            .diff(existing.as_deref().unwrap_or_default(), &args.content);
        let question = if existing.is_some() {
            format!("Overwrite {shown}?")
        } else {
            format!("Create {shown}?")
        };
        if !session.approve(Approval::Edit, &question).await {
            return Ok(DENIED.to_owned());
        }

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io_error(&session.display(parent), &e))?;
        }
        tokio::fs::write(&path, &args.content)
            .await
            .map_err(|e| io_error(&shown, &e))?;
        session.mark_read(&path);

        let lines = args.content.lines().count();
        if existing.is_some() {
            session
                .ui
                .tool_done(&format!("Wrote {shown} (+{added} -{removed})"));
            Ok(format!("Overwrote {shown} ({lines} lines)."))
        } else {
            session
                .ui
                .tool_done(&format!("Created {shown} ({lines} lines)"));
            Ok(format!("Created {shown} ({lines} lines)."))
        }
    }
}

// ------------------------------------------------------------------------ edit_file

pub struct EditFile(pub Arc<Session>);

#[derive(Deserialize)]
pub struct EditArgs {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

impl CodingTool for EditFile {
    type Args = EditArgs;

    const NAME: &'static str = "edit_file";
    const DESCRIPTION: &'static str = "Replace exact text in a file. `old_string` must match the \
        file exactly, including whitespace and indentation (copy it from read_file output, \
        without the line-number prefix), and must occur exactly once unless `replace_all` is \
        true; include a few surrounding lines to make it unique. Read the file first. The user \
        is asked to approve the edit.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path, relative to the working directory or absolute" },
                "old_string": { "type": "string", "description": "The exact text to replace" },
                "new_string": { "type": "string", "description": "The text to put in its place" },
                "replace_all": { "type": "boolean", "description": "Replace every occurrence instead of exactly one (default false)" }
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: EditArgs) -> Result<String, String> {
        let session = &self.0;
        let path = session.resolve(&args.path);
        let shown = session.display(&path);
        session.ui.tool_start("Update", &shown);

        session.ensure_fresh(&path, &shown)?;
        let old = read_text(&path, &shown).await?;
        let (new, count) = apply_edit(&old, &args, &shown)?;

        let (added, removed) = session.ui.diff(&old, &new);
        if !session
            .approve(Approval::Edit, &format!("Apply this edit to {shown}?"))
            .await
        {
            return Ok(DENIED.to_owned());
        }
        tokio::fs::write(&path, &new)
            .await
            .map_err(|e| io_error(&shown, &e))?;
        session.mark_read(&path);

        session
            .ui
            .tool_done(&format!("Updated {shown} (+{added} -{removed})"));
        let occurrences = if count == 1 {
            "1 occurrence".to_owned()
        } else {
            format!("{count} occurrences")
        };
        Ok(format!("Edited {shown}: replaced {occurrences}."))
    }
}

/// Apply an edit to `text`, returning the new text and how many occurrences were replaced.
fn apply_edit(text: &str, args: &EditArgs, shown: &str) -> Result<(String, usize), String> {
    if args.old_string.is_empty() {
        return Err("old_string is empty; use write_file to create a file".to_owned());
    }
    if args.old_string == args.new_string {
        return Err("old_string and new_string are identical; nothing would change".to_owned());
    }
    match text.matches(args.old_string.as_str()).count() {
        0 => Err(format!(
            "old_string was not found in {shown}. Copy the text exactly as it appears in \
             read_file output, including whitespace."
        )),
        1 => Ok((text.replacen(&args.old_string, &args.new_string, 1), 1)),
        count if args.replace_all => Ok((text.replace(&args.old_string, &args.new_string), count)),
        count => Err(format!(
            "old_string occurs {count} times in {shown}. Include more surrounding lines to make \
             it unique, or set replace_all to replace every occurrence."
        )),
    }
}

// ------------------------------------------------------------------------ list_dir

pub struct ListDir(pub Arc<Session>);

#[derive(Deserialize)]
pub struct ListArgs {
    path: Option<String>,
}

impl CodingTool for ListDir {
    type Args = ListArgs;

    const NAME: &'static str = "list_dir";
    const DESCRIPTION: &'static str = "List the files and subdirectories in a directory (default: \
        the working directory). Subdirectories end with `/`.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to list (default: the working directory)" }
            },
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    async fn run(&self, args: ListArgs) -> Result<String, String> {
        let session = &self.0;
        let path = session.resolve(args.path.as_deref().unwrap_or("."));
        let shown = session.display(&path);
        session.ui.tool_start("List", &shown);

        let mut reader = tokio::fs::read_dir(&path)
            .await
            .map_err(|e| io_error(&shown, &e))?;
        let mut entries = Vec::new();
        while let Some(entry) = reader
            .next_entry()
            .await
            .map_err(|e| io_error(&shown, &e))?
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            let is_dir = entry.file_type().await.is_ok_and(|kind| kind.is_dir());
            entries.push((!is_dir, if is_dir { format!("{name}/") } else { name }));
        }
        entries.sort();

        let total = entries.len();
        session.ui.tool_done(&format!(
            "{total} {}",
            if total == 1 { "entry" } else { "entries" }
        ));
        if total == 0 {
            return Ok(format!("{shown} is empty."));
        }
        let mut out: Vec<String> = entries
            .into_iter()
            .take(MAX_DIR_ENTRIES)
            .map(|(_, name)| name)
            .collect();
        if total > MAX_DIR_ENTRIES {
            out.push(format!("… and {} more", total - MAX_DIR_ENTRIES));
        }
        Ok(out.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(old: &str, new: &str, replace_all: bool) -> EditArgs {
        EditArgs {
            path: String::new(),
            old_string: old.to_owned(),
            new_string: new.to_owned(),
            replace_all,
        }
    }

    #[test]
    fn edits_replace_exactly_one_occurrence() {
        let text = "fn a() {}\nfn b() {}\n";
        assert_eq!(
            apply_edit(text, &edit("fn b", "fn c", false), "f").unwrap(),
            ("fn a() {}\nfn c() {}\n".to_owned(), 1)
        );
        // Ambiguous matches are refused unless replace_all is set.
        assert!(apply_edit(text, &edit("fn ", "pub fn ", false), "f").is_err());
        assert_eq!(
            apply_edit(text, &edit("fn ", "pub fn ", true), "f")
                .unwrap()
                .1,
            2
        );
        assert!(apply_edit(text, &edit("missing", "x", false), "f").is_err());
        assert!(apply_edit(text, &edit("fn a", "fn a", false), "f").is_err());
    }
}

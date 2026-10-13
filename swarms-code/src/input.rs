//! Reading the user's messages: line editing and history (rustyline), `\` line continuation,
//! and tab completion with inline hints for slash commands.

use std::{
    borrow::Cow,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use rustyline::{
    CompletionType, Config, Context, Editor, Helper,
    completion::{Completer, Pair},
    error::ReadlineError,
    highlight::Highlighter,
    hint::{Hint, Hinter},
    history::DefaultHistory,
    validate::Validator,
};

use crate::{
    effort::Effort,
    models,
    session::home_dir,
    ui::{Ui, dim},
};

pub const COMMANDS: [&str; 8] = [
    "/help", "/clear", "/model", "/effort", "/todos", "/exit", "/quit", "/reset",
];

pub enum Input {
    Line(String),
    /// ctrl+c at the prompt.
    Interrupted,
    /// ctrl+d at the prompt.
    Eof,
}

pub struct Reader {
    editor: Editor<SlashCommands, DefaultHistory>,
    history_path: Option<PathBuf>,
    example: &'static str,
}

impl Reader {
    pub fn new() -> Result<Self> {
        let config = Config::builder()
            .auto_add_history(false)
            .bracketed_paste(true)
            .completion_type(CompletionType::List)
            .max_history_size(1000)?
            .build();
        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(SlashCommands { placeholder: None }));
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos() as usize);
        let example = EXAMPLES[seed % EXAMPLES.len()];
        let history_path = home_dir().map(|home| home.join(".swarms-code").join("history"));
        if let Some(path) = &history_path {
            let _ = editor.load_history(path);
        }
        Ok(Self {
            editor,
            history_path,
            example,
        })
    }

    /// Read one message. A line ending in `\` continues on the next line.
    pub fn read(&mut self) -> Result<Input> {
        let mut message = String::new();
        loop {
            let prompt = Ui::prompt(!message.is_empty());
            if let Some(helper) = self.editor.helper_mut() {
                helper.placeholder = message.is_empty().then_some(self.example);
            }
            // rustyline blocks; let the runtime move other tasks off this thread meanwhile.
            match tokio::task::block_in_place(|| self.editor.readline(&prompt)) {
                Ok(line) => {
                    if let Some(continued) = line.strip_suffix('\\') {
                        message.push_str(continued);
                        message.push('\n');
                        continue;
                    }
                    message.push_str(&line);
                    if !message.trim().is_empty() {
                        let _ = self.editor.add_history_entry(message.as_str());
                    }
                    return Ok(Input::Line(message));
                },
                Err(ReadlineError::Interrupted) => return Ok(Input::Interrupted),
                Err(ReadlineError::Eof) => return Ok(Input::Eof),
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub fn save_history(&mut self) {
        if let Some(path) = &self.history_path {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = self.editor.save_history(path);
        }
    }
}

pub struct SlashCommands {
    /// Shown dimmed in an empty prompt, as in Claude Code; `None` on continuation lines.
    placeholder: Option<&'static str>,
}

/// Example requests for the empty-prompt placeholder; one is picked per session.
const EXAMPLES: [&str; 4] = [
    r#"Try "fix the failing tests""#,
    r#"Try "explain how this project is organized""#,
    r#"Try "add a --verbose flag to the CLI""#,
    r#"Try "find where errors are handled""#,
];

pub enum InputHint {
    /// The rest of a slash command; → accepts it.
    Command(String),
    /// The placeholder, which is never inserted.
    Placeholder(&'static str),
}

impl Hint for InputHint {
    fn display(&self) -> &str {
        match self {
            InputHint::Command(rest) => rest,
            InputHint::Placeholder(text) => text,
        }
    }

    fn completion(&self) -> Option<&str> {
        match self {
            InputHint::Command(rest) => Some(rest),
            InputHint::Placeholder(_) => None,
        }
    }
}

/// Completions for the text before the cursor: a slash command, or a model for `/model`.
/// Returns where the completed word starts and the candidates for it.
fn completions(line: &str, pos: usize) -> (usize, Vec<String>) {
    if pos < line.len() {
        return (0, Vec::new());
    }
    let (start, word, choices): (usize, &str, Vec<&str>) =
        if let Some(arg) = line.strip_prefix("/model ") {
            let choices = models::ANTHROPIC_MODELS
                .iter()
                .flat_map(|(id, alias, _)| [*alias, *id])
                .chain(["openrouter/"])
                .collect();
            ("/model ".len(), arg, choices)
        } else if let Some(arg) = line.strip_prefix("/effort ") {
            let choices = Effort::ALL
                .iter()
                .map(|level| level.as_str())
                .chain(["auto"]);
            ("/effort ".len(), arg, choices.collect())
        } else if line.starts_with('/') {
            (0, line, COMMANDS.to_vec())
        } else {
            return (0, Vec::new());
        };
    if word.contains(char::is_whitespace) {
        return (0, Vec::new());
    }
    let candidates = choices
        .into_iter()
        .filter(|choice| choice.starts_with(word) && *choice != word)
        .map(str::to_owned)
        .collect();
    (start, candidates)
}

impl Completer for SlashCommands {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (start, candidates) = completions(line, pos);
        let candidates = candidates
            .into_iter()
            .map(|candidate| Pair {
                display: candidate.clone(),
                replacement: candidate,
            })
            .collect();
        Ok((start, candidates))
    }
}

impl Hinter for SlashCommands {
    type Hint = InputHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<InputHint> {
        if line.is_empty() {
            return self.placeholder.map(InputHint::Placeholder);
        }
        let (start, candidates) = completions(line, pos);
        let typed = line.len() - start;
        candidates
            .first()
            .map(|candidate| InputHint::Command(candidate[typed..].to_owned()))
    }
}

impl Highlighter for SlashCommands {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(dim(hint))
    }
}

impl Validator for SlashCommands {}

impl Helper for SlashCommands {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_commands_and_model_names() {
        assert_eq!(completions("/cl", 3), (0, vec!["/clear".to_owned()]));
        assert_eq!(completions("/model so", 9), (7, vec!["sonnet".to_owned()]));
        assert_eq!(
            completions("/model claude-h", 15),
            (7, vec!["claude-haiku-5-5".to_owned()])
        );
        assert_eq!(
            completions("/model open", 11),
            (7, vec!["openrouter/".to_owned()])
        );
        assert_eq!(completions("/ef", 3), (0, vec!["/effort".to_owned()]));
        assert_eq!(
            completions("/effort m", 9),
            (8, vec!["medium".to_owned(), "max".to_owned()])
        );
        assert_eq!(completions("/effort x", 9), (8, vec!["xhigh".to_owned()]));
        // Nothing while editing mid-line, or for ordinary messages.
        assert!(completions("/model so", 3).1.is_empty());
        assert!(completions("fix the bug", 11).1.is_empty());
    }
}

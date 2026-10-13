//! Terminal rendering: the "You" and "Swarms Code" chat sections, tool activity, diffs, the
//! spinner and permission prompts.
//!
//! Every write goes through one lock shared with the spinner task, so a spinner frame never
//! lands in the middle of other output. In print mode (`-p`) all of this goes to stderr and
//! only the final answer is written to stdout.

use std::{
    borrow::Cow,
    fmt::Write as _,
    io::{self, IsTerminal, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Attribute, Color, ContentStyle, Stylize},
    terminal,
};
use similar::{ChangeTag, TextDiff};
use unicode_width::UnicodeWidthChar;

use crate::{
    lock,
    tools::todo::{Todo, TodoStatus},
    welcome::{self, Welcome},
};

pub const AGENT_NAME: &str = "Swarms Code";

// The interface uses two hues: Swarms red and greys. The greys come from the fixed
// 256-color ramp so they read the same on dark and light terminal themes.
/// Your section header and the input prompt.
const GREY: Color = Color::AnsiValue(247);
/// Secondary text: hints, tool summaries, unchanged diff lines.
const DIM_GREY: Color = Color::AnsiValue(243);
const SPINNER_FRAMES: [&str; 10] = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
const MAX_DIFF_LINES: usize = 40;
const OUTPUT_PREVIEW_LINES: usize = 4;

static COLOR: AtomicBool = AtomicBool::new(true);
static TRUECOLOR: AtomicBool = AtomicBool::new(false);

/// Swarms red (#DC0202, from the logo), the agent's accent color. Terminals without 24-bit
/// color get the nearest 256-color red.
pub fn accent() -> Color {
    if TRUECOLOR.load(Ordering::Relaxed) {
        Color::Rgb { r: 220, g: 2, b: 2 }
    } else {
        Color::AnsiValue(160)
    }
}

/// A lighter red for inline code, so code stands out from prose without reading as an error.
fn accent_soft() -> Color {
    if TRUECOLOR.load(Ordering::Relaxed) {
        Color::Rgb {
            r: 240,
            g: 96,
            b: 96,
        }
    } else {
        Color::AnsiValue(203)
    }
}

/// The user's answer to a permission prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Yes,
    /// Yes, and don't ask again for this kind of action this session.
    Always,
    No,
}

pub struct Ui {
    to_stderr: bool,
    /// The output is a terminal, so the spinner and prompts can be shown.
    live: bool,
    print_mode: bool,
    line: Arc<Mutex<LineState>>,
}

#[derive(Default)]
struct LineState {
    /// A spinner frame is on the current line and must be cleared before other output.
    spinner_drawn: bool,
}

impl Ui {
    pub fn new(print_mode: bool) -> Self {
        let live = if print_mode {
            io::stderr().is_terminal()
        } else {
            io::stdout().is_terminal()
        };
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        COLOR.store(live && !no_color, Ordering::Relaxed);
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        TRUECOLOR.store(
            matches!(colorterm.as_str(), "truecolor" | "24bit"),
            Ordering::Relaxed,
        );
        Self {
            to_stderr: print_mode,
            live,
            print_mode,
            line: Arc::default(),
        }
    }

    fn emit(&self, text: &str) {
        let mut line = lock(&self.line);
        write_out(self.to_stderr, line.spinner_drawn, text);
        line.spinner_drawn = false;
    }

    // ---------------------------------------------------------------- sections

    /// The welcome screen. On a terminal the invader marches in first; any key skips that.
    pub async fn welcome(&self, info: &Welcome<'_>) {
        let (columns, rows) =
            terminal::size().map_or((0, 0), |(c, r)| (usize::from(c), usize::from(r)));
        if columns < 40 {
            self.emit(&welcome::compact(info));
            return;
        }
        let width = (columns - 1).min(welcome::MAX_WIDTH);
        let frames: Vec<String> = (0..welcome::STEPS)
            .map(|step| welcome::render(info, width, step).join("\n"))
            .collect();
        let height = frames[0].lines().count();
        let animate = self.live && COLOR.load(Ordering::Relaxed) && rows > height + 2;
        if !animate {
            self.emit(&format!("\n{}\n", frames[welcome::STEPS - 1]));
            return;
        }
        self.emit(&format!("\n{}\n", frames[0]));
        let to_stderr = self.to_stderr;
        let played = tokio::task::spawn_blocking(move || play(&frames, height, to_stderr)).await;
        if !matches!(played, Ok(Ok(()))) {
            // Leave the terminal usable even if the animation couldn't run.
            let _ = terminal::disable_raw_mode();
            write_out(to_stderr, false, "\x1b[?25h");
        }
    }

    pub fn user_header(&self) {
        self.emit(&format!("\n{} {}\n", fg("●", GREY), fg_bold("You", GREY)));
    }

    pub fn agent_header(&self) {
        self.emit(&format!(
            "\n{} {}\n",
            fg("●", accent()),
            fg_bold(AGENT_NAME, accent())
        ));
    }

    /// The input prompt as `(plain, styled)`; both must have the same display width.
    pub fn prompt(continuation: bool) -> (String, String) {
        let symbol = if continuation { "…" } else { "›" };
        (
            format!("  {symbol} "),
            format!("  {} ", fg_bold(symbol, GREY)),
        )
    }

    /// Text the model wrote. In print mode the final answer goes to stdout instead.
    pub fn agent_message(&self, text: &str, is_final: bool) {
        if text.trim().is_empty() || (self.print_mode && is_final) {
            return;
        }
        self.emit(&render_markdown(text.trim(), term_width()));
    }

    pub fn help(&self) {
        let rows = [
            ("/help", "Show this help"),
            ("/clear", "Start a new conversation"),
            ("/model", "Pick a model from Anthropic or OpenRouter"),
            (
                "/model <name>",
                "Switch directly: opus, sonnet, haiku, or an OpenRouter ID",
            ),
            ("/effort", "Pick how much the model thinks before acting"),
            (
                "/effort <level>",
                "Set it directly: low, medium, high, xhigh, max, or auto",
            ),
            ("/todos", "Show the agent's todo list"),
            ("/exit", "Quit (or press ctrl+d)"),
        ];
        let mut text = format!("  {}\n", bold("Commands"));
        for (command, about) in rows {
            let _ = writeln!(text, "    {}  {about}", red(&format!("{command:<15}")));
        }
        let _ = write!(
            text,
            "\n  {}\n    End a line with \\ to keep typing on the next line; pasted text can span \
             lines.\n    ctrl+c interrupts the agent. At an empty prompt, press it twice to \
             quit.\n\n  {}\n    File edits and commands ask first: y = yes, a = yes for the rest \
             of the session, n = no.\n    Read-only commands (ls, cat, git status, grep, …) run \
             without asking.\n    Start with --auto-approve to skip every prompt.\n",
            bold("Input"),
            bold("Permissions"),
        );
        self.emit(&text);
    }

    // ---------------------------------------------------------------- notices

    pub fn info(&self, message: &str) {
        self.emit(&format!("  {}\n", dim(message)));
    }

    pub fn warn(&self, message: &str) {
        self.emit(&format!("  {} {message}\n", red("⚠")));
    }

    pub fn error(&self, message: &str) {
        self.emit(&format!("  {} {}\n", red("✗"), red(&sanitize(message))));
    }

    pub fn footer(&self, parts: &[String]) {
        self.emit(&format!(
            "\n  {}\n",
            dim(&format!("✻ {}", parts.join(" · ")))
        ));
    }

    // ---------------------------------------------------------------- tools

    /// The header line for a tool call, e.g. `⏺ Bash(cargo test)`.
    pub fn tool_start(&self, name: &str, detail: &str) {
        let mut lines = detail.lines();
        let first = lines.next().unwrap_or_default();
        let more = if lines.next().is_some() { " …" } else { "" };
        let room = term_width().saturating_sub(name.chars().count() + 8);
        let detail = truncate_width(&format!("{first}{more}"), room);
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!("({detail})")
        };
        self.emit(&format!("  {} {}{detail}\n", fg("⏺", accent()), bold(name)));
    }

    /// Show a command in full, wrapped, when [`Self::tool_start`] had to shorten it.
    pub fn full_command(&self, command: &str) {
        let width = term_width();
        let fits = !command.contains('\n')
            && textwrap::core::display_width(command) <= width.saturating_sub("Bash".len() + 9);
        if fits {
            return;
        }
        let gutter = format!("      {} ", dim("│"));
        let options = textwrap::Options::new(width.saturating_sub(1))
            .initial_indent(&gutter)
            .subsequent_indent(&gutter);
        let mut text = String::new();
        for line in sanitize(command).lines() {
            for wrapped in textwrap::wrap(line, &options) {
                let _ = writeln!(text, "{wrapped}");
            }
        }
        self.emit(&text);
    }

    pub fn tool_done(&self, summary: &str) {
        self.emit(&format!("    {}  {}\n", dim("⎿"), dim(&sanitize(summary))));
    }

    pub fn tool_error(&self, message: &str) {
        let first = message.lines().next().unwrap_or_default();
        self.emit(&format!("    {}  {}\n", dim("⎿"), red(&sanitize(first))));
    }

    /// The first few lines of a command's output.
    pub fn tool_output(&self, output: &str) {
        let lines: Vec<&str> = output.lines().collect();
        if lines.is_empty() {
            self.tool_done("(no output)");
            return;
        }
        let width = term_width().saturating_sub(9);
        let mut text = String::new();
        for (index, line) in lines.iter().take(OUTPUT_PREVIEW_LINES).enumerate() {
            let gutter = if index == 0 { "⎿" } else { " " };
            let _ = writeln!(
                text,
                "    {}  {}",
                dim(gutter),
                dim(&truncate_width(line, width))
            );
        }
        if lines.len() > OUTPUT_PREVIEW_LINES {
            let hidden = lines.len() - OUTPUT_PREVIEW_LINES;
            let _ = writeln!(text, "       {}", dim(&format!("… +{hidden} lines")));
        }
        self.emit(&text);
    }

    /// Show a line diff between two versions of a file; returns (added, removed) line counts.
    pub fn diff(&self, old: &str, new: &str) -> (usize, usize) {
        let diff = TextDiff::configure()
            .timeout(Duration::from_millis(250))
            .diff_lines(old, new);
        let width = term_width().saturating_sub(16);
        let (mut added, mut removed, mut shown, mut hidden) = (0, 0, 0, 0);
        let mut text = String::new();
        for (index, group) in diff.grouped_ops(2).iter().enumerate() {
            if index > 0 && shown < MAX_DIFF_LINES {
                let _ = writeln!(text, "      {}", dim("   ⋮"));
            }
            for op in group {
                for change in diff.iter_changes(op) {
                    // Removed lines in red, added lines in the normal text color, context in grey.
                    let (sign, number, style): (_, _, fn(&str) -> String) = match change.tag() {
                        ChangeTag::Delete => {
                            removed += 1;
                            ("-", change.old_index(), red)
                        },
                        ChangeTag::Insert => {
                            added += 1;
                            ("+", change.new_index(), str::to_owned)
                        },
                        ChangeTag::Equal => (" ", change.new_index(), dim),
                    };
                    if shown == MAX_DIFF_LINES {
                        hidden += 1;
                        continue;
                    }
                    shown += 1;
                    let body = truncate_width(change.value().trim_end_matches(['\n', '\r']), width);
                    let line = format!("{:>5} {sign} {body}", number.map_or(0, |n| n + 1));
                    let line = style(&line);
                    let _ = writeln!(text, "      {line}");
                }
            }
        }
        if hidden > 0 {
            let _ = writeln!(
                text,
                "      {}",
                dim(&format!("… +{hidden} more diff lines"))
            );
        }
        self.emit(&text);
        (added, removed)
    }

    pub fn todos(&self, todos: &[Todo]) {
        if todos.is_empty() {
            self.tool_done("The todo list is empty");
            return;
        }
        let mut text = String::new();
        for (index, todo) in todos.iter().enumerate() {
            let gutter = if index == 0 { "⎿" } else { " " };
            let content = sanitize(&todo.content);
            let item = match todo.status {
                TodoStatus::Completed => paint(
                    &format!("☒ {content}"),
                    ContentStyle::new()
                        .with(DIM_GREY)
                        .attribute(Attribute::CrossedOut),
                ),
                TodoStatus::InProgress => fg_bold(&format!("◼ {content}"), accent()),
                TodoStatus::Pending => format!("☐ {content}"),
            };
            let _ = writeln!(text, "    {}  {item}", dim(gutter));
        }
        self.emit(&text);
    }

    // ---------------------------------------------------------------- interaction

    /// Let the user choose from `items` with a filterable list: type to filter, arrows to
    /// move, enter to choose, esc to cancel. Returns the chosen item's index.
    pub async fn pick(
        &self,
        heading: Heading,
        items: Vec<PickItem>,
        selected: usize,
    ) -> Option<usize> {
        if !self.live || items.is_empty() {
            return None;
        }
        let to_stderr = self.to_stderr;
        tokio::task::spawn_blocking(move || run_picker(&heading, &items, selected, to_stderr))
            .await
            .ok()
            .flatten()
    }

    /// Show a spinner until the returned guard is dropped. `None` when output isn't a terminal.
    pub fn spinner(&self, label: &'static str) -> Option<Spinner> {
        if !self.live {
            return None;
        }
        let active = Arc::new(AtomicBool::new(true));
        let task = tokio::spawn({
            let (active, line, to_stderr) =
                (Arc::clone(&active), Arc::clone(&self.line), self.to_stderr);
            async move {
                let started = Instant::now();
                for frame in SPINNER_FRAMES.iter().cycle() {
                    {
                        let mut state = lock(&line);
                        if !active.load(Ordering::Relaxed) {
                            break;
                        }
                        let status = format!(
                            "{label}… {}s · ctrl+c to interrupt",
                            started.elapsed().as_secs()
                        );
                        write_out(
                            to_stderr,
                            true,
                            &format!("  {} {}", fg(frame, accent()), dim(&status)),
                        );
                        state.spinner_drawn = true;
                    }
                    tokio::time::sleep(Duration::from_millis(120)).await;
                }
            }
        });
        Some(Spinner {
            active,
            line: Arc::clone(&self.line),
            to_stderr: self.to_stderr,
            task,
        })
    }

    /// Ask whether a tool may proceed and read a single key: y (or enter), a, or n (or esc).
    /// `always` describes what "don't ask again" covers; `None` hides that option.
    pub async fn ask_permission(&self, question: &str, always: Option<&str>) -> Answer {
        let mut text = format!("    {} {}\n", red("?"), bold(question));
        let _ = writeln!(text, "      {}  Yes", key("y"));
        if let Some(always) = always {
            let _ = writeln!(
                text,
                "      {}  Yes, and don't ask again for {always} this session",
                key("a")
            );
        }
        let _ = writeln!(
            text,
            "      {}  No, and tell {AGENT_NAME} what to do instead",
            key("n")
        );
        self.emit(&text);

        let allow_always = always.is_some();
        let answer = tokio::task::spawn_blocking(move || read_answer(allow_always))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(Answer::No);
        let result = match answer {
            Answer::Yes => fg("✓ Allowed", GREY),
            Answer::Always => fg(
                &format!("✓ Allowed {} for this session", always.unwrap_or_default()),
                GREY,
            ),
            Answer::No => red("✗ Denied"),
        };
        self.emit(&format!("      {result}\n"));
        answer
    }
}

/// Clears its spinner when dropped, including when the turn is interrupted.
pub struct Spinner {
    active: Arc<AtomicBool>,
    line: Arc<Mutex<LineState>>,
    to_stderr: bool,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Spinner {
    fn drop(&mut self) {
        let mut state = lock(&self.line);
        self.active.store(false, Ordering::Relaxed);
        if state.spinner_drawn {
            write_out(self.to_stderr, true, "");
            state.spinner_drawn = false;
        }
        self.task.abort();
    }
}

fn term_width() -> usize {
    terminal::size().map_or(100, |(columns, _)| usize::from(columns).max(40))
}

fn write_out(to_stderr: bool, clear_line: bool, text: &str) {
    let mut out: Box<dyn Write> = if to_stderr {
        Box::new(io::stderr().lock())
    } else {
        Box::new(io::stdout().lock())
    };
    if clear_line {
        let _ = out.write_all(b"\r\x1b[2K");
    }
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

/// Redraw the welcome panel through its animation frames, in place. Raw mode keeps typed keys
/// from echoing into the panel; any key skips to the last frame.
fn play(frames: &[String], height: usize, to_stderr: bool) -> io::Result<()> {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = terminal::disable_raw_mode();
            write_out(self.0, false, "\x1b[?25h");
        }
    }

    terminal::enable_raw_mode()?;
    let _restore = Restore(to_stderr);
    write_out(to_stderr, false, "\x1b[?25l");
    let draw = |frame: &str| {
        let mut text = format!("\x1b[{height}A");
        for line in frame.lines() {
            let _ = write!(text, "\r\x1b[2K{line}\r\n");
        }
        write_out(to_stderr, false, &text);
    };
    for frame in &frames[1..frames.len() - 1] {
        if event::poll(welcome::FRAME_DELAY)? && matches!(event::read()?, Event::Key(_)) {
            break;
        }
        draw(frame);
    }
    if let Some(last) = frames.last() {
        draw(last);
    }
    Ok(())
}

/// One choice in a picker.
pub struct PickItem {
    pub name: String,
    /// A short category, such as the provider.
    pub tag: String,
    pub detail: String,
    /// Extra text the filter matches, beyond what's shown.
    pub keywords: String,
}

/// The lines around a picker's list.
pub struct Heading {
    pub title: String,
    pub subtitle: String,
    pub footer: String,
    /// Shown in the empty filter box.
    pub placeholder: String,
}

const PICKER_ROWS: usize = 10;
const PICKER_NAME_WIDTH: usize = 44;
const PICKER_TAG_WIDTH: usize = 10;

struct Picker<'a> {
    items: &'a [PickItem],
    query: String,
    /// Indexes into `items` that match the query.
    matches: Vec<usize>,
    /// Position in `matches`.
    selected: usize,
    scroll: usize,
    rows: usize,
}

impl<'a> Picker<'a> {
    fn new(items: &'a [PickItem], selected: usize, rows: usize) -> Self {
        let mut picker = Self {
            items,
            query: String::new(),
            matches: (0..items.len()).collect(),
            selected: 0,
            scroll: 0,
            rows,
        };
        picker.move_by(isize::try_from(selected).unwrap_or(0));
        picker
    }

    /// Keep items matching every word of the query, in their original order.
    fn refilter(&mut self) {
        let terms: Vec<String> = self
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        self.matches = (0..self.items.len())
            .filter(|&index| {
                let item = &self.items[index];
                let text = format!(
                    "{} {} {} {}",
                    item.name, item.tag, item.detail, item.keywords
                )
                .to_lowercase();
                terms.iter().all(|term| text.contains(term.as_str()))
            })
            .collect();
        self.selected = 0;
        self.scroll = 0;
    }

    fn move_by(&mut self, delta: isize) {
        let last = self.matches.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + self.rows {
            self.scroll = self.selected + 1 - self.rows;
        }
    }

    fn chosen(&self) -> Option<usize> {
        self.matches.get(self.selected).copied()
    }

    fn height(&self) -> usize {
        self.rows + 4
    }

    /// The picker as lines, always [`Self::height`] of them, each under `width` columns.
    fn frame(&self, heading: &Heading, width: usize) -> Vec<String> {
        let name_width = self
            .items
            .iter()
            .map(|item| item.name.chars().count())
            .max()
            .unwrap_or(0)
            .min(PICKER_NAME_WIDTH);
        // Marker, gaps and a spare column at the end; the name gives way on narrow screens.
        let columns = width.saturating_sub(4 + 2 + PICKER_TAG_WIDTH + 2 + 1);
        let name_width = name_width.min(columns);
        let detail_width = columns - name_width;
        let pad = |text: &str, width: usize| {
            let text = truncate_width(text, width);
            let gap = width.saturating_sub(textwrap::core::display_width(&text));
            format!("{text}{}", " ".repeat(gap))
        };

        let mut lines = vec![
            format!(
                "  {}  {}",
                red_bold(&heading.title),
                dim(&truncate_width(
                    &heading.subtitle,
                    width.saturating_sub(heading.title.len() + 5)
                ))
            ),
            format!(
                "  {}",
                dim(&truncate_width(
                    "Type to filter · ↑↓ to move · enter to choose · esc to cancel",
                    width.saturating_sub(3)
                ))
            ),
        ];
        let query = if self.query.is_empty() {
            dim(&truncate_width(
                &heading.placeholder,
                width.saturating_sub(5),
            ))
        } else {
            truncate_width(&self.query, width.saturating_sub(5))
        };
        lines.push(format!("  {} {query}", fg_bold("›", GREY)));

        for row in 0..self.rows {
            let position = self.scroll + row;
            let Some(&index) = self.matches.get(position) else {
                lines.push(String::new());
                continue;
            };
            let item = &self.items[index];
            let name = pad(&item.name, name_width);
            let line = format!(
                "{}  {}  {}",
                if position == self.selected {
                    red_bold(&name)
                } else {
                    name
                },
                dim(&pad(&item.tag, PICKER_TAG_WIDTH)),
                dim(&truncate_width(&item.detail, detail_width)),
            );
            let marker = if position == self.selected {
                red_bold("❯")
            } else {
                " ".to_owned()
            };
            lines.push(format!("  {marker} {line}"));
        }

        let count = if self.matches.is_empty() {
            "No matches".to_owned()
        } else {
            format!("{} of {}", self.matches.len(), self.items.len())
        };
        let footer = format!("{count} · {}", heading.footer);
        lines.push(format!(
            "  {}",
            dim(&truncate_width(&footer, width.saturating_sub(3)))
        ));
        lines
    }
}

fn run_picker(
    heading: &Heading,
    items: &[PickItem],
    selected: usize,
    to_stderr: bool,
) -> Option<usize> {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = terminal::disable_raw_mode();
            write_out(self.0, false, "\x1b[?25h");
        }
    }

    let screen_rows = terminal::size().map_or(24, |(_, rows)| usize::from(rows));
    let rows = PICKER_ROWS
        .min(items.len())
        .min(screen_rows.saturating_sub(6))
        .max(3);
    let mut picker = Picker::new(items, selected, rows);
    terminal::enable_raw_mode().ok()?;
    let _restore = Restore(to_stderr);

    let height = picker.height();
    let mut drawn = false;
    let mut draw = |picker: &Picker<'_>| {
        let mut text = String::from("\x1b[?25l");
        if drawn {
            let _ = write!(text, "\x1b[{height}A");
        }
        for line in picker.frame(heading, term_width().saturating_sub(1)) {
            let _ = write!(text, "\r\x1b[2K{line}\r\n");
        }
        write_out(to_stderr, false, &text);
        drawn = true;
    };
    draw(&picker);

    let choice = loop {
        let Ok(event) = event::read() else {
            break None;
        };
        let Event::Key(key) = event else {
            draw(&picker);
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let page = isize::try_from(picker.rows).unwrap_or(1);
        match key.code {
            KeyCode::Esc => break None,
            KeyCode::Char('c') if ctrl => break None,
            KeyCode::Enter => break picker.chosen(),
            KeyCode::Up => picker.move_by(-1),
            KeyCode::Char('p') if ctrl => picker.move_by(-1),
            KeyCode::Down => picker.move_by(1),
            KeyCode::Char('n') if ctrl => picker.move_by(1),
            KeyCode::PageUp => picker.move_by(-page),
            KeyCode::PageDown => picker.move_by(page),
            KeyCode::Backspace => {
                picker.query.pop();
                picker.refilter();
            },
            KeyCode::Char(ch) if !ctrl => {
                picker.query.push(ch);
                picker.refilter();
            },
            _ => {},
        }
        draw(&picker);
    };
    // Erase the picker; the caller reports the outcome on a single line.
    write_out(to_stderr, false, &format!("\x1b[{height}A\r\x1b[J"));
    choice
}

fn read_answer(allow_always: bool) -> io::Result<Answer> {
    struct RawMode;
    impl Drop for RawMode {
        fn drop(&mut self) {
            let _ = terminal::disable_raw_mode();
        }
    }

    terminal::enable_raw_mode()?;
    let _raw = RawMode;
    // Discard keys typed while the agent was working so they can't answer the question.
    while event::poll(Duration::ZERO)? {
        event::read()?;
    }
    loop {
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(Answer::No);
        }
        match key.code {
            KeyCode::Char('y' | 'Y' | '1') | KeyCode::Enter => return Ok(Answer::Yes),
            KeyCode::Char('a' | 'A' | '2') if allow_always => return Ok(Answer::Always),
            KeyCode::Char('n' | 'N' | '3') | KeyCode::Esc => return Ok(Answer::No),
            _ => {},
        }
    }
}

// -------------------------------------------------------------------- styling helpers

fn paint(text: &str, style: ContentStyle) -> String {
    if COLOR.load(Ordering::Relaxed) {
        style.apply(text).to_string()
    } else {
        text.to_owned()
    }
}

pub fn fg(text: &str, color: Color) -> String {
    paint(text, ContentStyle::new().with(color))
}

pub fn fg_bold(text: &str, color: Color) -> String {
    paint(
        text,
        ContentStyle::new().with(color).attribute(Attribute::Bold),
    )
}

pub fn red(text: &str) -> String {
    fg(text, accent())
}

pub fn red_bold(text: &str) -> String {
    fg_bold(text, accent())
}

pub fn dim(text: &str) -> String {
    paint(text, ContentStyle::new().with(DIM_GREY))
}

pub fn bold(text: &str) -> String {
    paint(text, ContentStyle::new().attribute(Attribute::Bold))
}

fn key(name: &str) -> String {
    red_bold(name)
}

/// Expand tabs and drop other control characters, so tool output and model text can't move
/// the cursor or change terminal state.
fn sanitize(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut clean = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\t' => clean.push_str("    "),
            '\n' => clean.push('\n'),
            ch if ch.is_control() => {},
            ch => clean.push(ch),
        }
    }
    Cow::Owned(clean)
}

/// Cut a single line to `max` terminal columns, ending with `…` when shortened.
pub fn truncate_width(line: &str, max: usize) -> String {
    let line = sanitize(line);
    let line = line.lines().next().unwrap_or_default();
    if textwrap::core::display_width(line) <= max {
        return line.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    // Leave one column for the ellipsis.
    let mut out = String::new();
    let mut width = 0;
    for ch in line.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width >= max {
            break;
        }
        width += ch_width;
        out.push(ch);
    }
    out.push('…');
    out
}

/// A light markdown rendering for model replies: headings, bullets, `code` and **bold**,
/// wrapped to the terminal and indented under the agent's section header.
fn render_markdown(text: &str, width: usize) -> String {
    let width = width.saturating_sub(1).max(20);
    let mut out = String::new();
    let mut in_code_block = false;
    for raw in text.lines() {
        let raw = sanitize(raw);
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            let _ = writeln!(out, "  {} {}", dim("│"), truncate_width(&raw, width - 4));
            continue;
        }
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        let indent = " ".repeat(raw.len() - trimmed.len());
        let (body, first, rest) = if let Some(heading) = heading_text(trimmed) {
            (
                bold(&heading.replace("**", "")),
                "  ".to_owned(),
                "  ".to_owned(),
            )
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            (
                inline(item),
                format!("  {indent}• "),
                format!("  {indent}  "),
            )
        } else if let Some((marker, item)) = numbered_item(trimmed) {
            let hang = " ".repeat(marker.len() + 1);
            (
                inline(item),
                format!("  {indent}{marker} "),
                format!("  {indent}{hang}"),
            )
        } else {
            (
                inline(trimmed),
                format!("  {indent}"),
                format!("  {indent}"),
            )
        };
        let options = textwrap::Options::new(width)
            .initial_indent(&first)
            .subsequent_indent(&rest)
            .break_words(false);
        for line in textwrap::wrap(&body, options) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

fn heading_text(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|&ch| ch == '#').count();
    (1..=6)
        .contains(&hashes)
        .then(|| line[hashes..].strip_prefix(' '))
        .flatten()
}

fn numbered_item(line: &str) -> Option<(&str, &str)> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let rest = line[digits..].strip_prefix(". ")?;
    Some((&line[..=digits], rest))
}

/// Style `code` spans and **bold** runs.
fn inline(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix('`')
            && let Some(end) = after.find('`')
        {
            out.push_str(&fg(&after[..end], accent_soft()));
            rest = &after[end + 1..];
            continue;
        }
        if let Some(after) = rest.strip_prefix("**")
            && let Some(end) = after.find("**")
        {
            out.push_str(&bold(&after[..end]));
            rest = &after[end + 2..];
            continue;
        }
        // *italic*, but not a lone `*` as in `a * b`.
        if let Some(after) = rest.strip_prefix('*')
            && let Some(end) = after.find('*')
            && end > 0
            && !after.starts_with(char::is_whitespace)
            && !after[..end].ends_with(char::is_whitespace)
        {
            out.push_str(&paint(
                &after[..end],
                ContentStyle::new().attribute(Attribute::Italic),
            ));
            rest = &after[end + 1..];
            continue;
        }
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_renders_plainly_without_color() {
        COLOR.store(false, Ordering::Relaxed);
        let text = "## Summary\nFixed `parse()` in **two** *small* places:\n- one\n  - nested\n1. first\n```rust\nlet x = 1;\n```";
        assert_eq!(
            render_markdown(text, 80),
            "  Summary\n  Fixed parse() in two small places:\n  • one\n    • nested\n  1. first\n  │ let x = 1;\n"
        );
    }

    fn items(names: &[&str]) -> Vec<PickItem> {
        names
            .iter()
            .map(|name| PickItem {
                name: (*name).to_owned(),
                tag: if name.starts_with("openrouter/") {
                    "OpenRouter"
                } else {
                    "Anthropic"
                }
                .to_owned(),
                detail: "1M context".to_owned(),
                keywords: String::new(),
            })
            .collect()
    }

    #[test]
    fn picker_filters_on_every_word_and_scrolls() {
        let list = items(&[
            "claude-opus-5-5",
            "claude-sonnet-5-5",
            "openrouter/anthropic/claude-sonnet-5.5",
            "openrouter/openai/gpt-5.5",
            "openrouter/google/gemini-3.8-flash",
        ]);
        let mut picker = Picker::new(&list, 1, 3);
        assert_eq!(picker.chosen(), Some(1));

        picker.query = "sonnet openrouter".to_owned();
        picker.refilter();
        assert_eq!(picker.matches, vec![2]);

        picker.query.clear();
        picker.refilter();
        picker.move_by(4);
        assert_eq!((picker.chosen(), picker.scroll), (Some(4), 2));
        picker.move_by(-10);
        assert_eq!((picker.chosen(), picker.scroll), (Some(0), 0));

        picker.query = "nothing like this".to_owned();
        picker.refilter();
        assert_eq!(picker.chosen(), None);
    }

    #[test]
    fn picker_frames_keep_their_height_and_width() {
        let list = items(&[
            "claude-opus-5-5",
            "openrouter/x-ai/grok-4.7",
            "openrouter/qwen/qwen3-coder-next",
        ]);
        let heading = Heading {
            title: "Switch model".to_owned(),
            subtitle: "current: claude-opus-5-5".to_owned(),
            footer: "ANTHROPIC_API_KEY found".to_owned(),
            placeholder: "filter by name, provider or vendor".to_owned(),
        };
        for width in [120, 80, 50] {
            for query in ["", "grok", "no match"] {
                let mut picker = Picker::new(&list, 0, 4);
                picker.query = query.to_owned();
                picker.refilter();
                let frame = picker.frame(&heading, width);
                assert_eq!(frame.len(), picker.height());
                for line in &frame {
                    assert!(textwrap::core::display_width(line) < width, "{line}");
                }
            }
        }
    }

    #[test]
    fn truncation_respects_display_width() {
        assert_eq!(truncate_width("hello world", 6), "hello…");
        assert_eq!(truncate_width("short", 10), "short");
        assert_eq!(truncate_width("exactly", 7), "exactly");
        assert_eq!(truncate_width("no room", 0), "");
        assert_eq!(truncate_width("漢字漢字", 5), "漢字…");
        assert_eq!(truncate_width("a\tb\x1b[31m", 20), "a    b[31m");
    }
}

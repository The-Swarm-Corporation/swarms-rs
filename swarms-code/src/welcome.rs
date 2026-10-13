//! The welcome screen shown when the chat starts, laid out like Claude Code's: a red panel
//! with the title in its border, a space invader, the model and directory on the left, and
//! tips plus the API key status on the right.

use std::time::Duration;

use crate::ui::{AGENT_NAME, bold, dim, red, red_bold, truncate_width};

/// Frames in the boot animation; the last is the resting pose.
pub const STEPS: usize = 12;
pub const FRAME_DELAY: Duration = Duration::from_millis(80);
pub const MAX_WIDTH: usize = 100;
/// Narrower than this, the panel stacks into one column.
const TWO_COLUMN_WIDTH: usize = 72;

/// The two poses of the classic "crab" invader, marching like in the arcade game.
const SPRITES: [[&str; 8]; 2] = [
    [
        "  #     #  ",
        "   #   #   ",
        "  #######  ",
        " ## ### ## ",
        "###########",
        "# ####### #",
        "# #     # #",
        "   ## ##   ",
    ],
    [
        "  #     #  ",
        "#  #   #  #",
        "# ####### #",
        "### ### ###",
        "###########",
        " ######### ",
        "  #     #  ",
        " #       # ",
    ],
];
const SPRITE_WIDTH: usize = 11;

/// A line of text and the style to draw it in.
type StyledLine = (String, fn(&str) -> String);

pub enum KeySource {
    Environment,
    DotEnv,
}

pub struct Welcome<'a> {
    pub model: &'a str,
    /// The provider serving the model, e.g. "Anthropic".
    pub provider: &'a str,
    /// The effort level, when one is set.
    pub effort: Option<&'a str>,
    pub cwd: &'a str,
    /// The environment variable holding the provider's API key.
    pub key_var: &'a str,
    /// Where the key was found, or `None` if it's missing.
    pub key_source: Option<KeySource>,
    /// Other providers' keys that are set, for switching with /model.
    pub other_keys: Vec<&'a str>,
    /// The project instructions file in the working directory, if any.
    pub notes_file: Option<&'a str>,
    pub auto_approve: bool,
}

/// One sprite as four lines of half blocks, each line holding two rows of pixels.
fn sprite(pose: usize) -> [String; 4] {
    let rows = SPRITES[pose % 2];
    std::array::from_fn(|line| {
        let (top, bottom) = (rows[line * 2].as_bytes(), rows[line * 2 + 1].as_bytes());
        (0..SPRITE_WIDTH)
            .map(|x| match (top[x] == b'#', bottom[x] == b'#') {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            })
            .collect()
    })
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
}

/// `text` cut to `width` columns, styled, then padded to exactly `width` columns.
fn cell(text: &str, style: &dyn Fn(&str) -> String, width: usize, align: Align) -> String {
    let text = truncate_width(text, width);
    let gap = width.saturating_sub(textwrap::core::display_width(&text));
    let (before, after) = match align {
        Align::Left => (0, gap),
        Align::Center => (gap / 2, gap - gap / 2),
    };
    format!(
        "{}{}{}",
        " ".repeat(before),
        style(&text),
        " ".repeat(after)
    )
}

fn blank(width: usize) -> String {
    " ".repeat(width)
}

fn plain(text: &str) -> String {
    text.to_owned()
}

/// The invader at animation `step`: it marches in from the left, alternating poses, and comes
/// to rest centered.
fn invader(width: usize, step: usize) -> Vec<String> {
    let rest = width.saturating_sub(SPRITE_WIDTH) / 2;
    let start = 1.min(rest);
    let x = start + (rest - start) * step / (STEPS - 1);
    let pose = (STEPS - 1 - step) % 2;
    sprite(pose)
        .iter()
        .map(|line| {
            format!(
                "{}{}{}",
                " ".repeat(x),
                red(line),
                " ".repeat(width - x - SPRITE_WIDTH)
            )
        })
        .collect()
}

/// The left column: greeting, invader, model and directory.
fn identity(info: &Welcome<'_>, width: usize, step: usize) -> Vec<String> {
    let mut lines = vec![
        blank(width),
        cell(
            &format!("Welcome to {AGENT_NAME}!"),
            &bold,
            width,
            Align::Center,
        ),
        blank(width),
    ];
    lines.extend(invader(width, step));
    lines.push(blank(width));
    lines.push(cell(&model_line(info), &dim, width, Align::Center));
    lines.push(cell(
        &shorten_path(info.cwd, width),
        &dim,
        width,
        Align::Center,
    ));
    lines
}

/// The API key lines, prefixed with `indent`.
fn key_lines(info: &Welcome<'_>, indent: &str) -> Vec<StyledLine> {
    let mut lines: Vec<StyledLine> = Vec::new();
    match info.key_source {
        Some(KeySource::Environment) => {
            lines.push((
                format!("{indent}✓ {} found in your environment", info.key_var),
                plain,
            ));
        },
        Some(KeySource::DotEnv) => {
            lines.push((format!("{indent}✓ {} found in .env", info.key_var), plain));
        },
        None => {
            lines.push((format!("{indent}✗ {} not found", info.key_var), red));
            lines.push((
                format!("{indent}Export it or add it to .env, then restart"),
                dim,
            ));
        },
    }
    if !info.other_keys.is_empty() {
        lines.push((
            format!("{indent}Also set: {}", info.other_keys.join(", ")),
            dim,
        ));
    }
    lines
}

/// The right column: tips, then the API key status.
fn guide(info: &Welcome<'_>, width: usize) -> Vec<String> {
    let mut rows: Vec<StyledLine> = vec![
        (" Tips for getting started".to_owned(), red_bold),
        (
            " Ask for a fix, a feature, or an explanation".to_owned(),
            plain,
        ),
        match info.notes_file {
            Some(file) => (format!(" Using project notes from {file}"), plain),
            None => (
                " Add a SWARMS.md with notes for the agent".to_owned(),
                plain,
            ),
        },
        (
            " /model switches between Anthropic and OpenRouter".to_owned(),
            plain,
        ),
        (" /help for commands · ctrl+c interrupts".to_owned(), plain),
    ];
    if info.auto_approve {
        rows.push((" ⚠ Edits and commands run without asking".to_owned(), red));
    }
    rows.push((format!(" {} ", "─".repeat(width.saturating_sub(2))), red));
    rows.push((" API key".to_owned(), red_bold));
    rows.extend(key_lines(info, " "));
    rows.iter()
        .map(|(text, style)| cell(text, style, width, Align::Left))
        .collect()
}

/// The welcome panel at animation `step`, one string per terminal line, `width` columns wide.
pub fn render(info: &Welcome<'_>, width: usize, step: usize) -> Vec<String> {
    let version = format!(" v{}", env!("CARGO_PKG_VERSION"));
    let title_width = AGENT_NAME.len() + version.len();
    let top = format!(
        "{}{}{}{}",
        red("╭─── "),
        red_bold(AGENT_NAME),
        red(&version),
        red(&format!(
            " {}╮",
            "─".repeat(width.saturating_sub(title_width + 7))
        ))
    );
    let bottom = red(&format!("╰{}╯", "─".repeat(width - 2)));
    let side = red("│");

    let mut lines = vec![top];
    if width >= TWO_COLUMN_WIDTH {
        let left_width = (width - 3) * 9 / 20;
        let right_width = width - 3 - left_width;
        let left = identity(info, left_width, step);
        let mut right = vec![blank(right_width)];
        right.extend(guide(info, right_width));
        let height = left.len().max(right.len());
        for row in 0..height {
            let l = left.get(row).cloned().unwrap_or_else(|| blank(left_width));
            let r = right
                .get(row)
                .cloned()
                .unwrap_or_else(|| blank(right_width));
            lines.push(format!("{side}{l}{side}{r}{side}"));
        }
    } else {
        let inner = width - 2;
        let mut body = identity(info, inner, step);
        body.push(blank(inner));
        for (text, style) in key_lines(info, "") {
            body.push(cell(&text, &style, inner, Align::Center));
        }
        if info.auto_approve {
            let warning = "⚠ Edits and commands run without asking";
            body.push(cell(warning, &red, inner, Align::Center));
        }
        body.push(blank(inner));
        lines.extend(body.into_iter().map(|row| format!("{side}{row}{side}")));
    }
    lines.push(bottom);
    lines
}

/// For terminals too narrow for the panel.
pub fn compact(info: &Welcome<'_>) -> String {
    let mut text = format!(
        "\n{} {}\n{}\n",
        red("✻"),
        red_bold(AGENT_NAME),
        dim(&model_line(info))
    );
    for (line, style) in key_lines(info, "") {
        text.push_str(&style(&line));
        text.push('\n');
    }
    text
}

/// The model, its provider, and the effort when one is set.
fn model_line(info: &Welcome<'_>) -> String {
    match info.effort {
        Some(effort) => format!("{} · {} · {effort} effort", info.model, info.provider),
        None => format!("{} · {}", info.model, info.provider),
    }
}

/// Keep the end of a long path, which is the part that identifies the project.
fn shorten_path(path: &str, width: usize) -> String {
    let length = path.chars().count();
    if length <= width {
        return path.to_owned();
    }
    let tail: String = path.chars().skip(length - (width - 1)).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(key_source: Option<KeySource>) -> Welcome<'static> {
        Welcome {
            model: "claude-opus-5-5",
            provider: "Anthropic",
            effort: None,
            cwd: "~/code/project",
            key_var: "ANTHROPIC_API_KEY",
            key_source,
            other_keys: vec!["OPENAI_API_KEY"],
            notes_file: None,
            auto_approve: false,
        }
    }

    #[test]
    fn sprite_packs_two_pixel_rows_per_line() {
        assert_eq!(
            sprite(0),
            ["  ▀▄   ▄▀  ", " ▄█▀███▀█▄ ", "█▀███████▀█", "▀ ▀▄▄ ▄▄▀ ▀"]
        );
    }

    #[test]
    fn every_frame_has_the_same_shape() {
        // The animation redraws in place, so each frame must have the same lines and widths.
        for width in [100, 80, 60, 41] {
            for source in [None, Some(KeySource::DotEnv)] {
                let info = info(source);
                let first = render(&info, width, 0);
                for step in 0..STEPS {
                    let frame = render(&info, width, step);
                    assert_eq!(frame.len(), first.len());
                    for line in &frame {
                        assert_eq!(textwrap::core::display_width(line), width, "{line}");
                    }
                }
            }
        }
    }

    #[test]
    fn says_whether_the_key_was_found() {
        let text = |source| render(&info(source), 100, STEPS - 1).join("\n");
        assert!(text(Some(KeySource::DotEnv)).contains("ANTHROPIC_API_KEY found in .env"));
        assert!(text(Some(KeySource::Environment)).contains("found in your environment"));
        assert!(text(None).contains("ANTHROPIC_API_KEY not found"));
        // The narrow layout keeps the key status.
        assert!(render(&info(None), 50, 0).join("\n").contains("not found"));
    }

    #[test]
    fn long_paths_keep_their_end() {
        assert_eq!(
            shorten_path("~/a/very/long/path/project", 12),
            "…ath/project"
        );
        assert_eq!(shorten_path("~/short", 12), "~/short");
    }
}

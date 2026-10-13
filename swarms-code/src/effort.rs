//! Effort: how much the model thinks and how thoroughly it works. Higher levels handle harder
//! problems and take longer and cost more; lower ones are quicker and cheaper.
//!
//! Claude takes it as `output_config.effort`, OpenRouter as `reasoning.effort` (translated
//! for each model), and OpenAI as `reasoning_effort`. Left unset, each model uses its own
//! default.

use std::fmt;

use crate::models::{Backend, CatalogModel, Resolved};

/// The output limit at `xhigh` and `max` when `--max-tokens` isn't given: thinking counts
/// toward it, and a smaller one cuts deep reasoning off mid-thought.
pub const ROOMY_MAX_TOKENS: u64 = 64_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum Effort {
    Low,
    Medium,
    High,
    #[value(name = "xhigh")]
    XHigh,
    Max,
}

impl Effort {
    pub const ALL: [Effort; 5] = [
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::XHigh,
        Effort::Max,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Effort::Low => "Quickest and cheapest; thinks briefly",
            Effort::Medium => "Balanced speed and depth",
            Effort::High => "Thinks more for harder problems",
            Effort::XHigh => "Extra depth for difficult coding and long tasks",
            Effort::Max => "The deepest thinking; slowest and most expensive",
        }
    }

    pub fn parse(text: &str) -> Option<Effort> {
        Effort::ALL
            .into_iter()
            .find(|level| level.as_str().eq_ignore_ascii_case(text.trim()))
    }

    /// Whether this level needs [`ROOMY_MAX_TOKENS`] of output.
    pub fn needs_room(self) -> bool {
        self >= Effort::XHigh
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What `/effort` takes: a level, or `auto` (also `default`) for the model's own default.
pub fn parse_choice(text: &str) -> Result<Option<Effort>, String> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("auto") || text.eq_ignore_ascii_case("default") {
        return Ok(None);
    }
    Effort::parse(text).map(Some).ok_or_else(|| {
        format!("Unknown effort '{text}'. Use low, medium, high, xhigh, max, or auto.")
    })
}

/// The effort levels a model accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Support {
    /// Empty when the model has no effort setting.
    pub levels: Vec<Effort>,
    /// The level the model uses when none is set, when known.
    pub default: Option<Effort>,
}

/// What `model` accepts. `entry` is its listing in OpenRouter's catalog, when it has one.
pub fn support(model: &Resolved, entry: Option<&CatalogModel>) -> Support {
    let all = Effort::ALL.to_vec();
    match model.backend {
        Backend::Anthropic => Support {
            levels: all,
            default: anthropic_default(&model.id),
        },
        Backend::OpenRouter => match entry {
            Some(entry) => Support {
                levels: entry.efforts().unwrap_or_default(),
                default: entry.default_effort(),
            },
            // OpenRouter's routers (`openrouter/auto`), or the catalog couldn't load:
            // OpenRouter maps levels a model lacks to the nearest one it has.
            None => Support {
                levels: all,
                default: None,
            },
        },
        Backend::OpenAI => Support {
            levels: vec![Effort::Low, Effort::Medium, Effort::High, Effort::XHigh],
            default: None,
        },
        Backend::DeepSeek => Support {
            levels: Vec::new(),
            default: None,
        },
    }
}

/// The documented defaults of Anthropic's current models.
fn anthropic_default(id: &str) -> Option<Effort> {
    match id {
        "claude-opus-5-5" | "claude-haiku-5-5" => Some(Effort::Medium),
        "claude-sonnet-5-5" => Some(Effort::High),
        _ => None,
    }
}

/// The level to send for `wanted`: itself if the model takes it, else the nearest level
/// below it, else the lowest one. `None` when the model has no effort setting.
pub fn fit(wanted: Effort, levels: &[Effort]) -> Option<Effort> {
    levels
        .iter()
        .copied()
        .filter(|level| *level <= wanted)
        .max()
        .or_else(|| levels.iter().copied().min())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::resolve;

    #[test]
    fn parses_levels_and_auto() {
        assert_eq!(parse_choice("XHigh"), Ok(Some(Effort::XHigh)));
        assert_eq!(parse_choice(" low "), Ok(Some(Effort::Low)));
        assert_eq!(parse_choice("auto"), Ok(None));
        assert_eq!(parse_choice("default"), Ok(None));
        assert!(parse_choice("extreme").unwrap_err().contains("xhigh"));
    }

    #[test]
    fn fits_the_nearest_level_a_model_takes() {
        let gpt = [Effort::Low, Effort::Medium, Effort::High, Effort::XHigh];
        assert_eq!(fit(Effort::Max, &gpt), Some(Effort::XHigh));
        assert_eq!(fit(Effort::High, &gpt), Some(Effort::High));
        assert_eq!(
            fit(Effort::Low, &[Effort::High, Effort::Max]),
            Some(Effort::High)
        );
        assert_eq!(fit(Effort::High, &[]), None);
    }

    #[test]
    fn support_depends_on_the_provider() {
        let none = |_: &str| false;
        let opus = support(&resolve("opus", none).unwrap(), None);
        assert_eq!(opus.levels, Effort::ALL);
        assert_eq!(opus.default, Some(Effort::Medium));

        let catalog: Vec<CatalogModel> = serde_json::from_str(
            r#"[
            {"id": "openai/gpt-5.5", "name": "GPT-5.5", "supported_parameters": ["tools", "reasoning", "reasoning_effort"],
             "reasoning": {"supported_efforts": ["xhigh", "high", "medium", "low", "none"], "default_effort": "medium"}},
            {"id": "deepseek/deepseek-r1", "name": "R1", "supported_parameters": ["tools", "reasoning"],
             "reasoning": {"mandatory": true}},
            {"id": "z/levels", "name": "Levels", "supported_parameters": ["tools", "reasoning_effort"]}
        ]"#,
        )
        .unwrap();
        let openrouter = resolve("openrouter/openai/gpt-5.5", none).unwrap();
        let gpt = support(&openrouter, Some(&catalog[0]));
        assert_eq!(
            gpt.levels,
            [Effort::Low, Effort::Medium, Effort::High, Effort::XHigh]
        );
        assert_eq!(gpt.default, Some(Effort::Medium));
        // Reasoning without effort levels, and effort without a list of them.
        assert!(support(&openrouter, Some(&catalog[1])).levels.is_empty());
        assert_eq!(support(&openrouter, Some(&catalog[2])).levels, Effort::ALL);

        let deepseek = resolve("deepseek/deepseek-chat", |var: &str| {
            var == "DEEPSEEK_API_KEY"
        })
        .unwrap();
        assert!(support(&deepseek, None).levels.is_empty());
    }
}

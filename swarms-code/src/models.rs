//! Choosing a model: Claude through Anthropic's API, any of OpenRouter's models through
//! OpenRouter, short aliases, OpenRouter's live catalog, and the settings saved between
//! sessions.
//!
//! Names route like this:
//! - `claude-opus-5-5` or `anthropic/claude-opus-5-5`: Anthropic, with `ANTHROPIC_API_KEY`.
//! - `openrouter/<id>`, or an OpenRouter ID such as `anthropic/claude-opus-5.5` or
//!   `google/gemini-3.8-flash`: OpenRouter, with `OPENROUTER_API_KEY`.
//! - `openai/...` and `deepseek/...`: OpenRouter, unless that provider's own key is set.

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use swarms_rs::llm::provider::{
    anthropic::Anthropic,
    any::{AnyModel, Route},
    openrouter::{OPENROUTER_API_BASE, OpenRouter},
};

use crate::{effort::Effort, session::home_dir};

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// The default when the only key available is OpenRouter's.
pub const DEFAULT_OPENROUTER_MODEL: &str = "openrouter/anthropic/claude-opus-5.5";
const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com";
/// Sent to OpenRouter so requests are credited to Swarms Code in its app rankings.
const APP_NAME: &str = "Swarms Code";
const APP_URL: &str = "https://github.com/The-Swarm-Corporation/swarms-rs";

/// Claude models served by Anthropic's own API: model ID, alias, description.
pub const ANTHROPIC_MODELS: [(&str, &str, &str); 4] = [
    ("claude-opus-5-5", "opus", "Claude Opus 5.5 · the default"),
    (
        "claude-sonnet-5-5",
        "sonnet",
        "Claude Sonnet 5.5 · fast and capable",
    ),
    (
        "claude-haiku-5-5",
        "haiku",
        "Claude Haiku 5.5 · fastest, lowest cost",
    ),
    (
        "claude-fable-5-1",
        "fable",
        "Claude Fable 5.1 · most capable, highest cost",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Anthropic,
    OpenRouter,
    OpenAI,
    DeepSeek,
}

impl Backend {
    pub const ALL: [Backend; 4] = [
        Backend::Anthropic,
        Backend::OpenRouter,
        Backend::OpenAI,
        Backend::DeepSeek,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Backend::Anthropic => "Anthropic",
            Backend::OpenRouter => "OpenRouter",
            Backend::OpenAI => "OpenAI",
            Backend::DeepSeek => "DeepSeek",
        }
    }

    pub fn key_var(self) -> &'static str {
        match self {
            Backend::Anthropic => "ANTHROPIC_API_KEY",
            Backend::OpenRouter => "OPENROUTER_API_KEY",
            Backend::OpenAI => "OPENAI_API_KEY",
            Backend::DeepSeek => "DEEPSEEK_API_KEY",
        }
    }
}

/// A model name resolved to the provider that will serve it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub backend: Backend,
    /// The provider's own ID for the model.
    pub id: String,
    /// The name shown and saved, e.g. `claude-opus-5-5` or `openrouter/openai/gpt-5.5`.
    pub name: String,
}

/// Resolve what the user typed: an alias, an Anthropic ID, or an OpenRouter ID. `has_key`
/// says whether an API key variable is set.
pub fn resolve(input: &str, has_key: impl Fn(&str) -> bool) -> Result<Resolved, String> {
    let input = input.trim();
    let input = ANTHROPIC_MODELS
        .iter()
        .find(|(_, alias, _)| alias.eq_ignore_ascii_case(input))
        .map_or(input, |(id, _, _)| *id);
    let route = Route::parse(input).map_err(|_| {
        format!(
            "Unknown model '{input}'. Use an alias (opus, sonnet, haiku, fable), an Anthropic \
             model such as claude-sonnet-5-5, or an OpenRouter model such as \
             openrouter/openai/gpt-5.5. Type /model to pick from a list."
        )
    })?;
    let direct = |backend, id: String, name| Resolved { backend, id, name };
    Ok(match route {
        // Anthropic's own IDs use dashes (claude-opus-5-5); dotted versions are OpenRouter's.
        Route::Anthropic(id) if id.contains('.') => via_openrouter(format!("anthropic/{id}")),
        Route::Anthropic(id) => direct(Backend::Anthropic, id.clone(), id),
        Route::OpenAI(id) if !has_key(Backend::OpenAI.key_var()) => {
            via_openrouter(format!("openai/{id}"))
        },
        Route::OpenAI(id) => direct(Backend::OpenAI, id.clone(), format!("openai/{id}")),
        Route::DeepSeek(id) if !has_key(Backend::DeepSeek.key_var()) => {
            via_openrouter(format!("deepseek/{id}"))
        },
        Route::DeepSeek(id) => direct(Backend::DeepSeek, id.clone(), format!("deepseek/{id}")),
        Route::OpenRouter(id) => via_openrouter(id),
    })
}

fn via_openrouter(id: String) -> Resolved {
    // OpenRouter's own routers (`openrouter/auto`) already carry the prefix.
    let name = if id.starts_with("openrouter/") {
        id.clone()
    } else {
        format!("openrouter/{id}")
    };
    Resolved {
        backend: Backend::OpenRouter,
        id,
        name,
    }
}

/// The model to use when none was chosen: Claude Opus 5.5, through OpenRouter if that's the
/// only key available.
pub fn default_model(has_key: impl Fn(&str) -> bool) -> &'static str {
    if !has_key(Backend::Anthropic.key_var()) && has_key(Backend::OpenRouter.key_var()) {
        DEFAULT_OPENROUTER_MODEL
    } else {
        DEFAULT_MODEL
    }
}

/// Build the swarms-rs model for `model`, at `effort` if given. `env` reads environment
/// variables.
pub fn connect(
    model: &Resolved,
    effort: Option<Effort>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<AnyModel, String> {
    let var = model.backend.key_var();
    let key = env(var).ok_or_else(|| {
        format!(
            "{} needs the {var} environment variable. Export it, or put it in a .env file in \
             the working directory.",
            model.name
        )
    })?;
    let effort = effort.map(Effort::as_str);
    Ok(match model.backend {
        Backend::Anthropic => {
            let base = env("ANTHROPIC_BASE_URL").unwrap_or_else(|| ANTHROPIC_API_BASE.to_owned());
            let client = Anthropic::from_url(base, key).set_model(&model.id);
            AnyModel::Anthropic(match effort {
                Some(effort) => client.with_effort(effort),
                None => client,
            })
        },
        Backend::OpenRouter => {
            let client = OpenRouter::from_url(openrouter_base(&env), key)
                .set_model(&model.id)
                .with_app_name(APP_NAME)
                .with_app_url(APP_URL);
            AnyModel::OpenRouter(match effort {
                Some(effort) => client.with_reasoning_effort(effort),
                None => client,
            })
        },
        Backend::OpenAI | Backend::DeepSeek => {
            match AnyModel::from_model_name(&model.name).map_err(|error| error.to_string())? {
                AnyModel::OpenAI(client) if model.backend == Backend::OpenAI => {
                    AnyModel::OpenAI(match effort {
                        Some(effort) => client.with_reasoning_effort(effort),
                        None => client,
                    })
                },
                other => other,
            }
        },
    })
}

fn openrouter_base(env: &impl Fn(&str) -> Option<String>) -> String {
    env("OPENROUTER_API_BASE").unwrap_or_else(|| OPENROUTER_API_BASE.to_owned())
}

/// Whether an environment variable is set to something.
pub fn has_key(var: &str) -> bool {
    env_var(var).is_some()
}

pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

// ---------------------------------------------------------------------------- catalog

/// One of OpenRouter's models, from its public catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    created: Option<u64>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    supported_parameters: Option<Vec<String>>,
    #[serde(default)]
    top_provider: Option<TopProvider>,
    #[serde(default)]
    pricing: Option<Pricing>,
    /// Present for models that reason.
    #[serde(default)]
    reasoning: Option<Reasoning>,
}

#[derive(Debug, Clone, Deserialize)]
struct TopProvider {
    max_completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Reasoning {
    /// Levels such as "low" and "xhigh", plus OpenRouter-only ones such as "none".
    supported_efforts: Option<Vec<String>>,
    default_effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Pricing {
    /// US dollars per token, as a decimal string.
    prompt: Option<String>,
    completion: Option<String>,
}

impl CatalogModel {
    /// The most output tokens to request: the provider's cap, and at most a quarter of the
    /// context window so the conversation keeps room to grow.
    pub fn output_limit(&self) -> Option<u64> {
        let cap = self
            .top_provider
            .as_ref()
            .and_then(|p| p.max_completion_tokens);
        let share = self.context_length.map(|context| context / 4);
        match (cap, share) {
            (Some(cap), Some(share)) => Some(cap.min(share)),
            (cap, share) => cap.or(share),
        }
    }

    /// Context size and price, e.g. "1M context · $4/$20 per M tokens".
    pub fn summary(&self) -> String {
        let mut text = String::new();
        if let Some(context) = self.context_length {
            let size = if context >= 1_000_000 {
                format!("{}M", context / 1_000_000)
            } else {
                format!("{}K", context / 1000)
            };
            let _ = write!(text, "{size} context");
        }
        let per_million = |price: &Option<String>| {
            price
                .as_deref()
                .and_then(|price| price.parse::<f64>().ok())
                .map(|price| price * 1e6)
        };
        let pricing = self.pricing.as_ref();
        if let (Some(input), Some(output)) = (
            pricing.and_then(|p| per_million(&p.prompt)),
            pricing.and_then(|p| per_million(&p.completion)),
        ) {
            if !text.is_empty() {
                text.push_str(" · ");
            }
            if input == 0.0 && output == 0.0 {
                text.push_str("free");
            } else {
                let _ = write!(text, "{}/{} per M tokens", dollars(input), dollars(output));
            }
        }
        text
    }

    /// The effort levels the model lists, or every level for a model that takes an effort
    /// without listing levels. `None` when it has no effort setting.
    pub fn efforts(&self) -> Option<Vec<Effort>> {
        let listed = self
            .reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.supported_efforts.as_ref());
        match listed {
            Some(listed) => {
                let mut levels: Vec<Effort> = listed
                    .iter()
                    .filter_map(|level| Effort::parse(level))
                    .collect();
                levels.sort_unstable();
                levels.dedup();
                Some(levels)
            },
            None if self.supports("reasoning_effort") => Some(Effort::ALL.to_vec()),
            None => None,
        }
    }

    pub fn default_effort(&self) -> Option<Effort> {
        self.reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.default_effort.as_deref())
            .and_then(Effort::parse)
    }

    fn supports(&self, parameter: &str) -> bool {
        self.supported_parameters
            .as_ref()
            .is_some_and(|parameters| parameters.iter().any(|p| p == parameter))
    }

    fn supports_tools(&self) -> bool {
        self.supports("tools")
    }
}

fn dollars(amount: f64) -> String {
    let text = format!("{amount:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("${text}")
}

/// Fetch OpenRouter's catalog: the models Swarms Code can drive, newest first. The catalog is
/// public, so no API key is sent.
pub async fn fetch_catalog() -> Result<Vec<CatalogModel>> {
    #[derive(Deserialize)]
    struct Listing {
        data: Vec<CatalogModel>,
    }
    let url = format!("{}/models", openrouter_base(&env_var).trim_end_matches('/'));
    let listing: Listing = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?
        .get(&url)
        .send()
        .await
        .with_context(|| format!("couldn't reach {url}"))?
        .error_for_status()?
        .json()
        .await
        .context("unexpected response from OpenRouter's model list")?;
    Ok(usable(listing.data))
}

/// Keep models that support tool calling and answer interactively (OpenRouter's `:batch`
/// variants are for offline jobs), newest first.
fn usable(mut models: Vec<CatalogModel>) -> Vec<CatalogModel> {
    models.retain(|model| model.supports_tools() && !model.id.ends_with(":batch"));
    models.sort_by_key(|model| std::cmp::Reverse(model.created.unwrap_or(0)));
    models
}

/// `model`'s listing in the catalog, or why Swarms Code can't use it. OpenRouter's own
/// routers (`openrouter/auto`) have no listing.
pub fn check_against_catalog<'a>(
    model: &Resolved,
    catalog: &'a [CatalogModel],
) -> Result<Option<&'a CatalogModel>, String> {
    match catalog.iter().find(|entry| entry.id == model.id) {
        Some(entry) => Ok(Some(entry)),
        None if model.id.starts_with("openrouter/") => Ok(None),
        // The catalog only holds tool-capable models; a near miss is worth suggesting.
        None => {
            let base = model.id.split(':').next().unwrap_or_default();
            let hint = catalog
                .iter()
                .find(|entry| entry.id.starts_with(base) || base.starts_with(&entry.id))
                .map(|entry| format!(" Did you mean openrouter/{}?", entry.id))
                .unwrap_or_default();
            Err(format!(
                "OpenRouter has no model '{}' that supports tool calling, which Swarms Code \
                 needs.{hint} Type /model to pick from the list.",
                model.id
            ))
        },
    }
}

// ---------------------------------------------------------------------------- saved settings

fn settings_path() -> Option<PathBuf> {
    home_dir().map(|home| home.join(".swarms-code").join("settings.json"))
}

/// A setting saved by `/model` (`model`) or `/effort` (`effort`), if any.
pub fn setting(key: &str) -> Option<String> {
    setting_at(&settings_path()?, key)
}

/// Save a setting for next time; `None` removes it.
pub fn save_setting(key: &str, value: Option<&str>) -> Result<()> {
    let path = settings_path().context("couldn't find your home directory")?;
    save_setting_at(&path, key, value)
}

fn setting_at(path: &Path, key: &str) -> Option<String> {
    let settings: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    settings[key].as_str().map(str::to_owned)
}

fn save_setting_at(path: &Path, key: &str, value: Option<&str>) -> Result<()> {
    // Keep any other settings in the file.
    let mut settings = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
        .unwrap_or_default();
    match value {
        Some(value) => settings.insert(key.to_owned(), Value::String(value.to_owned())),
        None => settings.remove(key),
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string_pretty(&settings)? + "\n")
        .with_context(|| format!("couldn't write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(set: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |var| set.contains(&var)
    }

    fn route(input: &str, set: &'static [&'static str]) -> (Backend, String, String) {
        let resolved = resolve(input, keys(set)).unwrap();
        (resolved.backend, resolved.id, resolved.name)
    }

    #[test]
    fn names_route_to_anthropic_or_openrouter() {
        let anthropic = |id: &str| (Backend::Anthropic, id.to_owned(), id.to_owned());
        let openrouter = |id: &str| {
            (
                Backend::OpenRouter,
                id.to_owned(),
                format!("openrouter/{id}"),
            )
        };

        assert_eq!(route("opus", &[]), anthropic("claude-opus-5-5"));
        assert_eq!(route("Sonnet", &[]), anthropic("claude-sonnet-5-5"));
        assert_eq!(
            route("claude-haiku-5-5", &[]),
            anthropic("claude-haiku-5-5")
        );
        assert_eq!(
            route("anthropic/claude-opus-5-5", &[]),
            anthropic("claude-opus-5-5")
        );
        // Dotted Claude versions are OpenRouter's IDs.
        assert_eq!(
            route("anthropic/claude-opus-5.5", &[]),
            openrouter("anthropic/claude-opus-5.5")
        );
        assert_eq!(
            route("openrouter/anthropic/claude-opus-5.5", &[]),
            openrouter("anthropic/claude-opus-5.5")
        );
        assert_eq!(
            route("google/gemini-3.8-flash", &[]),
            openrouter("google/gemini-3.8-flash")
        );
        assert_eq!(route("openrouter/auto", &[]).2, "openrouter/auto");
        // OpenAI and DeepSeek go through OpenRouter unless their own key is set.
        assert_eq!(route("openai/gpt-5.5", &[]), openrouter("openai/gpt-5.5"));
        assert_eq!(route("gpt-5.5", &[]), openrouter("openai/gpt-5.5"));
        assert_eq!(
            route("openai/gpt-5.5", &["OPENAI_API_KEY"]).0,
            Backend::OpenAI
        );
        assert_eq!(
            route("deepseek/deepseek-chat", &[]),
            openrouter("deepseek/deepseek-chat")
        );
        assert!(resolve("mystery", keys(&[])).is_err());
    }

    #[test]
    fn default_prefers_anthropic_then_openrouter() {
        assert_eq!(default_model(keys(&[])), DEFAULT_MODEL);
        assert_eq!(
            default_model(keys(&["ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"])),
            DEFAULT_MODEL
        );
        assert_eq!(
            default_model(keys(&["OPENROUTER_API_KEY"])),
            DEFAULT_OPENROUTER_MODEL
        );
        // The OpenRouter default must itself resolve to OpenRouter.
        assert_eq!(route(DEFAULT_OPENROUTER_MODEL, &[]).0, Backend::OpenRouter);
    }

    #[test]
    fn missing_keys_are_reported_by_name() {
        let model = resolve("openrouter/openai/gpt-5.5", keys(&[])).unwrap();
        let error = connect(&model, None, |_| None).err().unwrap();
        assert!(error.contains("OPENROUTER_API_KEY"), "{error}");
        assert!(connect(&model, Some(Effort::High), |_| Some("key".to_owned())).is_ok());
    }

    fn catalog() -> Vec<CatalogModel> {
        serde_json::from_str::<Vec<CatalogModel>>(
            r#"[
            {"id": "anthropic/claude-opus-5.5", "name": "Anthropic: Claude Opus 5.5", "created": 3,
             "context_length": 1000000, "supported_parameters": ["tools", "max_tokens"],
             "top_provider": {"max_completion_tokens": 128000},
             "pricing": {"prompt": "0.000004", "completion": "0.00002"}},
            {"id": "anthropic/claude-opus-5.5:batch", "name": "Batch", "created": 3,
             "supported_parameters": ["tools"]},
            {"id": "deepseek/deepseek-r1", "name": "DeepSeek: R1", "created": 1,
             "context_length": 64000, "supported_parameters": ["tools"],
             "top_provider": {"max_completion_tokens": 16000},
             "pricing": {"prompt": "0.0000004", "completion": "0.000002"}},
            {"id": "some/chat-only", "name": "No tools", "created": 9, "supported_parameters": null},
            {"id": "free/model", "name": "Free", "created": 2, "context_length": 32768,
             "supported_parameters": ["tools"], "top_provider": null,
             "pricing": {"prompt": "0", "completion": "0"}}
        ]"#,
        )
        .map(usable)
        .unwrap()
    }

    #[test]
    fn catalog_keeps_interactive_tool_models_newest_first() {
        let ids: Vec<_> = catalog().into_iter().map(|model| model.id).collect();
        assert_eq!(
            ids,
            [
                "anthropic/claude-opus-5.5",
                "free/model",
                "deepseek/deepseek-r1"
            ]
        );
    }

    #[test]
    fn output_limits_and_summaries_come_from_the_catalog() {
        let models = catalog();
        let opus = &models[0];
        assert_eq!(opus.output_limit(), Some(128_000));
        assert_eq!(opus.summary(), "1M context · $4/$20 per M tokens");
        let r1 = &models[2];
        assert_eq!(r1.output_limit(), Some(16_000));
        assert_eq!(r1.summary(), "64K context · $0.4/$2 per M tokens");
        assert_eq!(models[1].output_limit(), Some(8192));
        assert_eq!(models[1].summary(), "32K context · free");
    }

    #[test]
    fn unknown_openrouter_models_are_caught_with_a_suggestion() {
        let models = catalog();
        let known = resolve("openrouter/deepseek/deepseek-r1", keys(&[])).unwrap();
        let entry = check_against_catalog(&known, &models).unwrap().unwrap();
        assert_eq!(entry.id, "deepseek/deepseek-r1");
        let router = resolve("openrouter/auto", keys(&[])).unwrap();
        assert!(check_against_catalog(&router, &models).unwrap().is_none());
        let typo = resolve("openrouter/anthropic/claude-opus-5.5-preview", keys(&[])).unwrap();
        let error = check_against_catalog(&typo, &models).unwrap_err();
        assert!(
            error.contains("Did you mean openrouter/anthropic/claude-opus-5.5?"),
            "{error}"
        );
    }

    #[test]
    fn settings_round_trip_and_keep_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        assert_eq!(setting_at(&path, "model"), None);
        save_setting_at(&path, "model", Some("sonnet")).unwrap();
        fs::write(&path, r#"{"model": "sonnet", "theme": "dark"}"#).unwrap();
        save_setting_at(&path, "model", Some("openrouter/openai/gpt-5.5")).unwrap();
        save_setting_at(&path, "effort", Some("high")).unwrap();
        assert_eq!(
            setting_at(&path, "model").as_deref(),
            Some("openrouter/openai/gpt-5.5")
        );
        assert_eq!(setting_at(&path, "effort").as_deref(), Some("high"));
        save_setting_at(&path, "effort", None).unwrap();
        assert_eq!(setting_at(&path, "effort"), None);
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("\"theme\": \"dark\"")
        );
    }
}

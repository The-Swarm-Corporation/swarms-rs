//! Swarms Code: a coding agent for the terminal, built on swarms-rs.

mod agent;
mod effort;
mod input;
mod models;
mod prompt;
mod session;
mod system_prompt;
mod tools;
mod ui;
mod welcome;

use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use anyhow::{Context as _, Result, bail};
use clap::Parser;

use crate::{
    agent::{Ending, Engine, TurnOutcome},
    effort::Effort,
    input::{Input, Reader},
    models::Backend,
    session::Session,
    ui::{Heading, PickItem, Ui},
    welcome::{KeySource, Welcome},
};

// Doc comments here are the CLI's help text, not rustdoc.
#[allow(clippy::doc_markdown)]
#[derive(Parser)]
#[command(
    name = "swarms-code",
    version,
    about = "Swarms Code: a coding agent for your terminal, built on swarms-rs",
    after_help = "Run without a prompt for an interactive chat; in the chat, /model switches \
                  models and /effort sets how hard they think. Claude models need \
                  ANTHROPIC_API_KEY and OpenRouter models need OPENROUTER_API_KEY, from the \
                  environment or a .env file in the working directory."
)]
struct Cli {
    /// Run one prompt, print the final answer to stdout, and exit (progress goes to stderr)
    #[arg(short, long, value_name = "PROMPT")]
    print: Option<String>,

    /// Model for this session: an alias (opus, sonnet, haiku, fable), an Anthropic model such as
    /// claude-sonnet-5-5, or an OpenRouter model such as openrouter/openai/gpt-5.5 [default:
    /// $SWARMS_CODE_MODEL, else your last /model choice, else claude-opus-5-5]
    #[arg(short, long)]
    model: Option<String>,

    /// How much the model thinks before acting: higher is more thorough, slower and costlier
    /// [default: your last /effort choice, else the model's own default]
    #[arg(short, long, value_name = "LEVEL")]
    effort: Option<Effort>,

    /// Directory to work in [default: the current directory]
    #[arg(short = 'C', long = "cwd", value_name = "DIR")]
    cwd: Option<PathBuf>,

    /// Let file edits and shell commands run without asking for approval
    #[arg(long)]
    auto_approve: bool,

    /// Maximum output tokens for each model call [default: 32000, or 64000 at xhigh and max
    /// effort]
    #[arg(long, value_name = "N")]
    max_tokens: Option<u64>,

    /// Maximum model calls for one message before handing control back to you
    #[arg(long, default_value_t = 100, value_name = "N")]
    max_steps: usize,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let cwd = match working_dir(cli.cwd.as_deref()) {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("swarms-code: {error:#}");
            return ExitCode::FAILURE;
        },
    };
    // Before the runtime starts any threads, since this sets environment variables.
    let from_dotenv = load_env_file(&cwd);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("swarms-code: couldn't start the async runtime: {error}");
            return ExitCode::FAILURE;
        },
    };
    match runtime.block_on(run(cli, cwd, from_dotenv)) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("swarms-code: {error:#}");
            ExitCode::FAILURE
        },
    }
}

async fn run(cli: Cli, cwd: PathBuf, from_dotenv: Vec<String>) -> Result<ExitCode> {
    // --model, then $SWARMS_CODE_MODEL, then the model last chosen with /model, then the
    // default for whichever keys are set.
    let model = cli
        .model
        .or_else(|| models::env_var("SWARMS_CODE_MODEL"))
        .or_else(|| {
            models::setting("model").filter(|saved| models::resolve(saved, models::has_key).is_ok())
        })
        .unwrap_or_else(|| models::default_model(models::has_key).to_owned());
    // --effort, then the level last chosen with /effort; otherwise the model decides.
    let effort = cli
        .effort
        .or_else(|| models::setting("effort").and_then(|saved| Effort::parse(&saved)));
    let print_mode = cli.print.is_some();
    let interactive = !print_mode && io::stdin().is_terminal();
    let ui = Ui::new(print_mode);
    let session = Arc::new(Session::new(cwd, ui, cli.auto_approve, interactive));
    let mut engine = Engine::new(
        Arc::clone(&session),
        &model,
        cli.max_tokens,
        effort,
        cli.max_steps,
    )?;

    if let Some(prompt) = cli.print {
        if let Some(problem) = engine.problem() {
            bail!("{problem}");
        }
        if let Some(warning) = engine.prepare().await {
            session.ui.warn(&warning);
        }
        return print_mode_turn(&mut engine, &session, &prompt).await;
    }
    chat(&mut engine, &session, &from_dotenv).await?;
    Ok(ExitCode::SUCCESS)
}

/// The interactive chat: read a message, run the agent, repeat.
async fn chat(engine: &mut Engine, session: &Session, from_dotenv: &[String]) -> Result<()> {
    let ui = &session.ui;
    welcome(engine, session, from_dotenv).await;
    if let Some(warning) = engine.prepare().await {
        ui.warn(&warning);
    }
    let mut reader = Reader::new()?;
    let mut exit_armed = false;
    let mut show_header = true;

    loop {
        if show_header {
            ui.user_header();
        }
        show_header = true;
        let line = match reader.read()? {
            Input::Line(line) => line,
            Input::Eof => break,
            Input::Interrupted if exit_armed => break,
            Input::Interrupted => {
                exit_armed = true;
                show_header = false;
                ui.info("Press ctrl+c again to exit");
                continue;
            },
        };
        exit_armed = false;
        let message = line.trim();
        if message.is_empty() {
            show_header = false;
            continue;
        }
        if message.starts_with('/') {
            if run_command(message, engine, session).await == Flow::Exit {
                break;
            }
            continue;
        }

        ui.agent_header();
        let finished = tokio::select! {
            result = engine.run_turn(message) => Some(result),
            _ = tokio::signal::ctrl_c() => None,
        };
        match finished {
            Some(Ok(outcome)) => ui.footer(&summary(&outcome)),
            Some(Err(error)) => ui.error(&format!("{error:#}")),
            None => {
                engine.note_interrupted();
                ui.error("Interrupted · tell Swarms Code what to do instead");
            },
        }
    }

    reader.save_history();
    ui.info("Bye!");
    Ok(())
}

/// Show the welcome screen, including whether the model's API key was found.
async fn welcome(engine: &Engine, session: &Session, from_dotenv: &[String]) {
    let backend = engine.backend();
    let key_var = backend.key_var();
    let key_source = models::has_key(key_var).then(|| {
        if from_dotenv.iter().any(|name| name == key_var) {
            KeySource::DotEnv
        } else {
            KeySource::Environment
        }
    });
    let cwd = session::tilde(&session.cwd);
    let info = Welcome {
        model: engine.model(),
        provider: backend.name(),
        effort: engine.applied_effort().map(Effort::as_str),
        cwd: &cwd,
        key_var,
        key_source,
        other_keys: Backend::ALL
            .iter()
            .map(|backend| backend.key_var())
            .filter(|var| *var != key_var && models::has_key(var))
            .collect(),
        notes_file: prompt::notes_file(&session.cwd),
        auto_approve: session.auto_approve(),
    };
    session.ui.welcome(&info).await;
}

/// `-p`: one turn with no prompts; the final answer is the only thing on stdout.
async fn print_mode_turn(engine: &mut Engine, session: &Session, prompt: &str) -> Result<ExitCode> {
    let finished = tokio::select! {
        result = engine.run_turn(prompt) => Some(result),
        _ = tokio::signal::ctrl_c() => None,
    };
    let Some(outcome) = finished else {
        session.ui.error("Interrupted");
        return Ok(ExitCode::from(130));
    };
    let outcome = outcome?;
    session.ui.footer(&summary(&outcome));
    if let Some(answer) = &outcome.answer {
        println!("{}", answer.trim());
    }
    Ok(if outcome.ending == Ending::Finished {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

#[derive(PartialEq, Eq)]
enum Flow {
    Continue,
    Exit,
}

async fn run_command(line: &str, engine: &mut Engine, session: &Session) -> Flow {
    let ui = &session.ui;
    let (name, arg) = line
        .split_once(char::is_whitespace)
        .map_or((line, ""), |(name, arg)| (name, arg.trim()));
    match name {
        "/help" | "/?" => ui.help(),
        "/clear" | "/reset" => {
            engine.clear();
            ui.info("Started a new conversation.");
        },
        "/model" | "/models" if arg.is_empty() => pick_model(engine, ui).await,
        "/model" | "/models" => switch_model(engine, ui, arg).await,
        "/effort" if arg.is_empty() => pick_effort(engine, ui).await,
        "/effort" => match effort::parse_choice(arg) {
            Ok(choice) => set_effort(engine, ui, choice).await,
            Err(problem) => ui.error(&problem),
        },
        "/todos" => ui.todos(&session.todos()),
        "/exit" | "/quit" | "/q" => return Flow::Exit,
        _ => ui.error(&format!(
            "Unknown command {name}. Type /help to see the commands."
        )),
    }
    Flow::Continue
}

/// `/model`: choose from Anthropic's models and OpenRouter's catalog.
async fn pick_model(engine: &mut Engine, ui: &Ui) {
    let catalog = {
        let _spinner = ui.spinner("Loading models");
        engine.catalog().await
    };
    let catalog = catalog
        .inspect_err(|error| {
            ui.warn(&format!(
                "Couldn't load OpenRouter's models ({error:#}); showing Anthropic's."
            ));
        })
        .ok();

    let mut items: Vec<PickItem> = models::ANTHROPIC_MODELS
        .iter()
        .map(|(id, alias, about)| PickItem {
            name: (*id).to_owned(),
            tag: Backend::Anthropic.name().to_owned(),
            detail: format!("{about} · /model {alias}"),
            keywords: (*alias).to_owned(),
        })
        .collect();
    items.extend(
        catalog
            .iter()
            .flat_map(|catalog| catalog.iter())
            .map(|model| PickItem {
                name: format!("openrouter/{}", model.id),
                tag: Backend::OpenRouter.name().to_owned(),
                detail: model.summary(),
                keywords: model.name.clone(),
            }),
    );
    let names: Vec<String> = items.iter().map(|item| item.name.clone()).collect();
    let current = names
        .iter()
        .position(|name| name == engine.model())
        .unwrap_or(0);
    let key_status = |backend: Backend| {
        let mark = if models::has_key(backend.key_var()) {
            "✓"
        } else {
            "✗"
        };
        format!("{} {mark}", backend.key_var())
    };
    let heading = Heading {
        title: "Switch model".to_owned(),
        subtitle: format!("current: {}", engine.model()),
        footer: format!(
            "{} · {}",
            key_status(Backend::Anthropic),
            key_status(Backend::OpenRouter)
        ),
        placeholder: "filter by name, provider or vendor".to_owned(),
    };
    match ui.pick(heading, items, current).await {
        Some(index) => switch_model(engine, ui, &names[index]).await,
        None => ui.info(&format!("Kept {}.", engine.model())),
    }
}

/// `/model <name>`: switch, and remember the choice for next time.
async fn switch_model(engine: &mut Engine, ui: &Ui, name: &str) {
    let switched = {
        let _spinner = ui.spinner("Checking the model");
        engine.set_model(name).await
    };
    match switched {
        Ok(note) => {
            let saved = models::save_setting("model", Some(engine.model()));
            ui.info(&format!(
                "Switched to {} via {}.{} The conversation carries over.",
                engine.model(),
                engine.backend().name(),
                if saved.is_ok() {
                    " Saved as your default."
                } else {
                    ""
                }
            ));
            if let Some(note) = note {
                ui.warn(&note);
            }
            if let Err(error) = saved {
                ui.warn(&format!("Couldn't save it as your default: {error:#}"));
            }
        },
        Err(error) => ui.error(&format!("{error:#}")),
    }
}

/// `/effort`: choose how much the model thinks, from the levels it takes.
async fn pick_effort(engine: &mut Engine, ui: &Ui) {
    let support = {
        let _spinner =
            (engine.backend() == Backend::OpenRouter).then(|| ui.spinner("Checking the model"));
        engine.effort_support().await
    };
    if support.levels.is_empty() {
        ui.info(&format!("{} has no effort setting.", engine.model()));
        return;
    }
    let mut choices = vec![None];
    choices.extend(support.levels.iter().copied().map(Some));
    let items = choices
        .iter()
        .map(|choice| match choice {
            None => PickItem {
                name: "auto".to_owned(),
                tag: String::new(),
                detail: support.default.map_or_else(
                    || "The model's own default".to_owned(),
                    |level| format!("The model's own default, {level}"),
                ),
                keywords: "default".to_owned(),
            },
            Some(level) => PickItem {
                name: level.to_string(),
                tag: if support.default == Some(*level) {
                    "default".to_owned()
                } else {
                    String::new()
                },
                detail: level.about().to_owned(),
                keywords: String::new(),
            },
        })
        .collect();
    let current = choices
        .iter()
        .position(|choice| *choice == engine.applied_effort())
        .unwrap_or(0);
    let heading = Heading {
        title: "Set effort".to_owned(),
        subtitle: format!(
            "current: {} · {}",
            effort_name(engine.applied_effort()),
            engine.model()
        ),
        footer: "Higher levels think longer: more thorough, slower, and costlier".to_owned(),
        placeholder: "filter levels".to_owned(),
    };
    match ui.pick(heading, items, current).await {
        Some(index) => set_effort(engine, ui, choices[index]).await,
        None => ui.info(&format!(
            "Kept {} effort.",
            effort_name(engine.applied_effort())
        )),
    }
}

/// `/effort <level>`: change the effort, and remember it for next time.
async fn set_effort(engine: &mut Engine, ui: &Ui, effort: Option<Effort>) {
    let changed = {
        let _spinner =
            (engine.backend() == Backend::OpenRouter).then(|| ui.spinner("Checking the model"));
        engine.set_effort(effort).await
    };
    match changed {
        Ok(note) => {
            let saved = models::save_setting("effort", effort.map(Effort::as_str));
            let what = match effort {
                Some(level) => format!("Effort set to {level}."),
                None => "Effort set to auto: each model uses its own default.".to_owned(),
            };
            ui.info(&format!(
                "{what}{}",
                if saved.is_ok() {
                    " Saved for next time."
                } else {
                    ""
                }
            ));
            if let Some(note) = note {
                ui.warn(&note);
            }
            if let Err(error) = saved {
                ui.warn(&format!("Couldn't save it for next time: {error:#}"));
            }
        },
        Err(error) => ui.error(&format!("{error:#}")),
    }
}

fn effort_name(effort: Option<Effort>) -> String {
    effort.map_or_else(|| "auto".to_owned(), |level| level.to_string())
}

fn summary(outcome: &TurnOutcome) -> Vec<String> {
    let mut parts = Vec::new();
    match outcome.ending {
        Ending::Finished => {},
        Ending::Declined => parts.push("Stopped · waiting for your direction".to_owned()),
        Ending::StepLimit => parts.push("Stopped at the step limit".to_owned()),
    }
    if outcome.tool_calls > 0 {
        let plural = if outcome.tool_calls == 1 { "" } else { "s" };
        parts.push(format!("{} tool call{plural}", outcome.tool_calls));
    }
    parts.push(format!("{:.1}s", outcome.elapsed.as_secs_f64()));
    if outcome.usage.input + outcome.usage.output > 0 {
        parts.push(format!(
            "{} in · {} out",
            tokens(outcome.usage.input),
            tokens(outcome.usage.output)
        ));
    }
    parts
}

// Rounded for display, so f64 precision is plenty.
#[allow(clippy::cast_precision_loss)]
fn tokens(count: u64) -> String {
    match count {
        0..1_000 => format!("{count}"),
        1_000..1_000_000 => format!("{:.1}k", count as f64 / 1e3),
        _ => format!("{:.2}M", count as f64 / 1e6),
    }
}

fn working_dir(requested: Option<&Path>) -> Result<PathBuf> {
    let dir = match requested {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir().context("couldn't read the current directory")?,
    };
    dir.canonicalize()
        .with_context(|| format!("{} is not an accessible directory", dir.display()))
}

/// Read API keys (and `SWARMS_CODE_MODEL`) from `.env` in the working directory, the
/// convention across swarms projects. Only those names are taken, and never over variables
/// already set: a repository's .env must not be able to redirect API traffic elsewhere
/// (e.g. by setting `ANTHROPIC_BASE_URL`).
///
/// Returns the names it set.
fn load_env_file(cwd: &Path) -> Vec<String> {
    let settings = env_file_settings(cwd);
    for (name, value) in &settings {
        // SAFETY: called from `main` before the async runtime (or any other thread) exists,
        // so nothing can read the environment concurrently.
        unsafe { std::env::set_var(name, value) };
    }
    settings.into_iter().map(|(name, _)| name).collect()
}

/// The `.env` entries [`load_env_file`] would set.
fn env_file_settings(cwd: &Path) -> Vec<(String, String)> {
    let Ok(entries) = dotenvy::from_path_iter(cwd.join(".env")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|(name, _)| name.ends_with("_API_KEY") || name == "SWARMS_CODE_MODEL")
        .filter(|(name, _)| std::env::var_os(name).is_none())
        .collect()
}

/// Lock a mutex, recovering the data if a panicking thread poisoned it.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_token_counts() {
        assert_eq!(tokens(950), "950");
        assert_eq!(tokens(12_345), "12.3k");
        assert_eq!(tokens(2_500_000), "2.50M");
    }

    #[test]
    fn env_file_only_supplies_missing_api_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".env"),
            // PATH is always set, so the file can't override it.
            "SWARMS_CODE_TEST_API_KEY=from-file\nANTHROPIC_BASE_URL=https://example.invalid\nPATH=/nowhere\n",
        )
        .unwrap();
        let settings = env_file_settings(dir.path());
        assert!(settings.contains(&(
            "SWARMS_CODE_TEST_API_KEY".to_owned(),
            "from-file".to_owned()
        )));
        assert!(
            !settings
                .iter()
                .any(|(name, _)| name == "ANTHROPIC_BASE_URL" || name == "PATH")
        );
    }
}

//! Regression tests for the SwarmsAgent run loop, driven by a scripted model so no API
//! key or network is needed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{BoxFuture, join_all};
use serde_json::json;
use swarms_macro::tool;
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::completion::{AssistantContent, Message, UserContent};
use swarms_rs::llm::request::{CompletionRequest, CompletionResponse};
use swarms_rs::llm::{CompletionError, Model};
use swarms_rs::structs::agent::Agent;

#[derive(Debug, thiserror::Error)]
#[error("test tool error")]
pub struct TestError;

/// Replies from a script (then "done"), optionally echoing the task back, and records each
/// request's prompt.
#[derive(Clone, Default)]
struct ScriptedModel {
    replies: Arc<Mutex<VecDeque<Result<Vec<AssistantContent>, String>>>>,
    prompts: Arc<Mutex<Vec<String>>>,
    tool_names: Arc<Mutex<Vec<String>>>,
    calls: Arc<AtomicUsize>,
    echo_task: bool,
}

impl ScriptedModel {
    fn new(replies: Vec<Result<Vec<AssistantContent>, String>>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into())),
            ..Default::default()
        }
    }

    fn echo() -> Self {
        Self {
            echo_task: true,
            ..Default::default()
        }
    }
}

fn first_user_text(history: &[Message]) -> String {
    history
        .iter()
        .find_map(|m| match m {
            Message::User { content } => content.iter().find_map(|c| match c {
                UserContent::Text(t) => Some(t.text.clone()),
                _ => None,
            }),
            _ => None,
        })
        .unwrap_or_default()
}

impl Model for ScriptedModel {
    type RawCompletionResponse = ();

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<()>, CompletionError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.prompts
                .lock()
                .unwrap()
                .push(format!("{:?}", request.prompt));
            *self.tool_names.lock().unwrap() =
                request.tools.iter().map(|t| t.name.clone()).collect();
            if self.echo_task {
                // The task text's length sets the delay, so later tasks can finish first.
                let task = first_user_text(&request.chat_history);
                let delay = task.rsplit(' ').next().unwrap_or_default().len() as u64;
                tokio::time::sleep(Duration::from_millis(10 * delay)).await;
                return Ok(CompletionResponse {
                    choice: vec![AssistantContent::text(format!("answer to [{task}]"))],
                    raw_response: (),
                });
            }
            tokio::task::yield_now().await;
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(vec![AssistantContent::text("done")]));
            match reply {
                Ok(choice) => Ok(CompletionResponse {
                    choice,
                    raw_response: (),
                }),
                Err(e) => Err(CompletionError::Provider(e)),
            }
        })
    }
}

static TEXT_THEN_TOOL_CALLS: AtomicUsize = AtomicUsize::new(0);
static EVALUATOR_SIBLING_CALLS: AtomicUsize = AtomicUsize::new(0);
static SEQUENTIAL_CALLS: AtomicUsize = AtomicUsize::new(0);

#[tool(description = "Counts calls")]
fn text_then_tool() -> Result<String, TestError> {
    TEXT_THEN_TOOL_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok("counted".to_string())
}

#[tool(description = "Counts calls")]
fn evaluator_sibling() -> Result<String, TestError> {
    EVALUATOR_SIBLING_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok("sibling result".to_string())
}

#[tool(description = "Counts calls")]
fn sequential_ok() -> Result<String, TestError> {
    SEQUENTIAL_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok("ok".to_string())
}

fn call(id: &str, name: &str, args: serde_json::Value) -> AssistantContent {
    AssistantContent::tool_call(id, name, args)
}

#[tokio::test]
async fn tool_call_after_text_is_executed() {
    let model = ScriptedModel::new(vec![Ok(vec![
        AssistantContent::text("Let me count."),
        call("1", "text_then_tool", json!({})),
    ])]);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .agent_name("core")
        .add_tool(TextThenTool)
        .build();

    let output = agent.run("count once".to_string()).await.unwrap();
    assert_eq!(TEXT_THEN_TOOL_CALLS.load(Ordering::SeqCst), 1);
    assert!(output.contains("[Tool result]: \"counted\""), "{output}");
}

#[tokio::test]
async fn failing_model_returns_error_instead_of_the_task() {
    let model = ScriptedModel::new(vec![Err("bad key".into()), Err("bad key".into())]);
    let calls = Arc::clone(&model.calls);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .retry_attempts(2)
        .build();

    let err = agent.run("do it".to_string()).await.unwrap_err();
    assert!(err.to_string().contains("bad key"), "{err}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn zero_retry_attempts_still_calls_the_model() {
    let model = ScriptedModel::new(vec![]);
    let calls = Arc::clone(&model.calls);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .retry_attempts(0)
        .build();

    let output = agent.run("do it".to_string()).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(output.contains("done"), "{output}");
}

#[tokio::test]
async fn run_multiple_tasks_does_not_deadlock_and_keeps_order() {
    let mut agent = SwarmsAgentBuilder::new_with_model(ScriptedModel::echo()).build();
    let tasks = vec![
        "task slowest".to_string(),
        "task mid".to_string(),
        "task f".to_string(),
    ];

    let results = tokio::time::timeout(Duration::from_secs(10), agent.run_multiple_tasks(tasks))
        .await
        .expect("run_multiple_tasks hung")
        .unwrap();

    assert_eq!(results.len(), 3);
    assert!(results[0].contains("task slowest]"), "{}", results[0]);
    assert!(results[1].contains("task mid]"), "{}", results[1]);
    assert!(results[2].contains("task f]"), "{}", results[2]);
}

#[tokio::test]
async fn concurrent_runs_on_one_agent_do_not_deadlock() {
    // Current-thread runtime: a DashMap guard held across the LLM call used to park the
    // only thread once another run wrote to the same shard.
    let agent = SwarmsAgentBuilder::new_with_model(ScriptedModel::new(vec![])).build();
    let runs = (0..16).map(|i| agent.run(format!("task {i}")));

    let results = tokio::time::timeout(Duration::from_secs(10), join_all(runs))
        .await
        .expect("concurrent runs deadlocked");
    assert!(results.iter().all(|r| r.is_ok()));
}

#[tokio::test]
async fn tool_results_next_to_task_evaluator_are_kept() {
    let model = ScriptedModel::new(vec![Ok(vec![
        call("1", "evaluator_sibling", json!({})),
        call("2", "task_evaluator", json!({"status": "Complete"})),
    ])]);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .add_tool(EvaluatorSibling)
        .max_loops(3)
        .build();

    let output = agent.run("finish".to_string()).await.unwrap();
    assert_eq!(EVALUATOR_SIBLING_CALLS.load(Ordering::SeqCst), 1);
    assert!(output.contains("sibling result"), "{output}");
    assert!(output.contains("[Tool name]: task_evaluator"), "{output}");
}

#[tokio::test]
async fn sequential_tool_errors_are_reported_without_rerunning_tools() {
    let model = ScriptedModel::new(vec![Ok(vec![
        call("1", "sequential_ok", json!({})),
        call("2", "does_not_exist", json!({})),
    ])]);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .add_tool(SequentialOk)
        .disable_concurrent_tool_call()
        .build();

    let output = agent.run("use tools".to_string()).await.unwrap();
    assert_eq!(SEQUENTIAL_CALLS.load(Ordering::SeqCst), 1);
    assert!(output.contains("Tool not found"), "{output}");
}

#[tokio::test]
async fn first_loop_after_planning_sends_a_user_prompt() {
    let model = ScriptedModel::new(vec![Ok(vec![AssistantContent::text("1. do it")])]);
    let prompts = Arc::clone(&model.prompts);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .enable_plan("Make a plan for:".to_string())
        .build();

    agent.run("the task".to_string()).await.unwrap();
    let prompts = prompts.lock().unwrap();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert!(prompts[1].contains("following your plan"), "{}", prompts[1]);
}

#[tokio::test]
async fn autosave_file_name_keeps_dots_in_agent_name() {
    let dir = tempfile::tempdir().unwrap();
    let agent = SwarmsAgentBuilder::new_with_model(ScriptedModel::new(vec![]))
        .agent_name("gpt-4.1-agent")
        .enable_autosave()
        .save_state_dir(dir.path().to_string_lossy())
        .build();

    agent.run("task A".to_string()).await.unwrap();
    agent.run("task B".to_string()).await.unwrap();

    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(
        names
            .iter()
            .all(|n| n.starts_with("gpt-4.1-agent_") && n.ends_with(".json")),
        "{names:?}"
    );
}

#[tokio::test]
async fn duplicate_tool_names_are_offered_once() {
    let model = ScriptedModel::new(vec![]);
    let tool_names = Arc::clone(&model.tool_names);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .add_tool(SequentialOk)
        .add_tool(SequentialOk)
        .build();

    agent.run("hello".to_string()).await.unwrap();
    let names = tool_names.lock().unwrap();
    assert_eq!(
        names.iter().filter(|n| *n == "sequential_ok").count(),
        1,
        "{names:?}"
    );
    assert_eq!(
        names.iter().filter(|n| *n == "task_evaluator").count(),
        1,
        "{names:?}"
    );
}

#[tool(description = "Returns a fixed sentinel value")]
fn stdout_probe_tool() -> Result<String, TestError> {
    Ok("stdout-probe-tool-result".to_string())
}

async fn run_stdout_probe(mode: &str) {
    let model = ScriptedModel::new(vec![Ok(vec![
        AssistantContent::text("stdout-probe-reply"),
        call("1", "stdout_probe_tool", json!({})),
    ])]);
    let mut builder = SwarmsAgentBuilder::new_with_model(model)
        .agent_name("ProbeAgent")
        .add_tool(StdoutProbeTool);
    match mode {
        "default" => {},
        "verbose_only" => {
            builder = builder.verbose(true);
        },
        "pretty" => {
            builder = builder.pretty_print_on(true);
        },
        other => panic!("unexpected stdout probe mode: {other}"),
    }
    let agent = builder.build();
    agent.run("stdout-probe-task".to_string()).await.unwrap();
}

#[tokio::test]
async fn stdout_behavior_matches_pretty_print_flag() {
    if let Ok(mode) = std::env::var("SWARMS_TEST_STDOUT_MODE") {
        run_stdout_probe(&mode).await;
        return;
    }

    let exe = std::env::current_exe().expect("current test binary path");

    for mode in ["default", "verbose_only"] {
        let output = std::process::Command::new(&exe)
            .args([
                "--exact",
                "stdout_behavior_matches_pretty_print_flag",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SWARMS_TEST_STDOUT_MODE", mode)
            .output()
            .expect("spawn stdout probe");
        assert!(output.status.success(), "mode={mode} failed: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        for forbidden in [
            "stdout-probe-reply",
            "stdout_probe_tool",
            "stdout-probe-tool-result",
            "stdout-probe-task",
            "🤖",
            "🔧",
            "✅",
            "🎯",
            "📋",
            "╭",
        ] {
            assert!(
                !stdout.contains(forbidden),
                "mode={mode} unexpectedly printed '{forbidden}' to stdout:\n{stdout}"
            );
        }
    }

    let pretty = std::process::Command::new(&exe)
        .args([
            "--exact",
            "stdout_behavior_matches_pretty_print_flag",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("SWARMS_TEST_STDOUT_MODE", "pretty")
        .output()
        .expect("spawn pretty stdout probe");
    assert!(pretty.status.success(), "pretty mode failed: {pretty:?}");
    let pretty_stdout = String::from_utf8_lossy(&pretty.stdout);
    for expected in [
        "╭",
        "ProbeAgent Output",
        "stdout-probe-reply",
        "Tool Execution",
        "stdout_probe_tool",
        "stdout-probe-tool-result",
        "Task Completed",
        "stdout-probe-task",
    ] {
        assert!(
            pretty_stdout.contains(expected),
            "pretty mode missing '{expected}' in stdout:\n{pretty_stdout}"
        );
    }
}

#[derive(Clone, Default)]
struct SharedBufferWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl std::io::Write for SharedBufferWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBufferWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn non_pretty_output_routes_to_debug_tracing() {
    if std::env::var_os("SWARMS_TEST_TRACING_PROBE").is_none() {
        let exe = std::env::current_exe().expect("current test binary path");
        let output = std::process::Command::new(&exe)
            .args([
                "--exact",
                "non_pretty_output_routes_to_debug_tracing",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SWARMS_TEST_TRACING_PROBE", "1")
            .output()
            .expect("spawn tracing probe");
        assert!(
            output.status.success(),
            "tracing probe failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let writer = SharedBufferWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(writer.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("set global tracing subscriber");

    let model = ScriptedModel::new(vec![Ok(vec![
        AssistantContent::text("stdout-probe-reply"),
        call("1", "stdout_probe_tool", json!({})),
    ])]);
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .agent_name("TraceAgent")
        .add_tool(StdoutProbeTool)
        .build();

    agent.run("stdout-probe-task".to_string()).await.unwrap();

    let logs = String::from_utf8(writer.buf.lock().unwrap().clone()).unwrap();
    for expected in [
        "Agent output",
        "stdout-probe-reply",
        "Tool executed",
        "stdout_probe_tool",
        "stdout-probe-tool-result",
        "Task completed",
        "stdout-probe-task",
    ] {
        assert!(
            logs.contains(expected),
            "debug tracing missing '{expected}':\n{logs}"
        );
    }
}

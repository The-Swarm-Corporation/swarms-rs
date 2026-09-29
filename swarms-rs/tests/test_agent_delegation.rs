//! Tests for sub-agents, handoffs and typed tool outputs in the conversation, driven by a
//! scripted model so no API key or network is needed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use serde_json::json;
use swarms_macro::tool;
use swarms_rs::agent::{SwarmsAgent, SwarmsAgentBuilder, ToolCallOutput};
use swarms_rs::llm::completion::{AssistantContent, Message, UserContent};
use swarms_rs::llm::request::{CompletionRequest, CompletionResponse};
use swarms_rs::llm::{CompletionError, Model};
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::conversation::{AgentConversation, Content, Role};

#[derive(Debug, thiserror::Error)]
#[error("test tool error")]
pub struct TestError;

/// Replies from a script (then `fallback`), recording what each request contained.
#[derive(Clone)]
struct ScriptedModel {
    replies: Arc<Mutex<VecDeque<Result<Vec<AssistantContent>, String>>>>,
    fallback: String,
    calls: Arc<AtomicUsize>,
    /// Tool names offered on each request.
    tools: Arc<Mutex<Vec<Vec<String>>>>,
    /// All text sent on each request (history plus prompt).
    requests: Arc<Mutex<Vec<String>>>,
}

impl ScriptedModel {
    fn new(replies: Vec<Result<Vec<AssistantContent>, String>>, fallback: &str) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into())),
            fallback: fallback.to_string(),
            calls: Arc::new(AtomicUsize::new(0)),
            tools: Arc::default(),
            requests: Arc::default(),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

fn message_text(message: &Message) -> String {
    match message {
        Message::User { content } => content
            .iter()
            .filter_map(|c| match c {
                UserContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Message::Assistant { content } => content
            .iter()
            .filter_map(|c| match c {
                AssistantContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

impl Model for ScriptedModel {
    type RawCompletionResponse = ();

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<()>, CompletionError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.tools
                .lock()
                .unwrap()
                .push(request.tools.iter().map(|t| t.name.clone()).collect());
            let mut text: Vec<String> = request.chat_history.iter().map(message_text).collect();
            text.push(message_text(&request.prompt));
            self.requests.lock().unwrap().push(text.join("\n"));

            tokio::task::yield_now().await;
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(vec![AssistantContent::text(self.fallback.clone())]));
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

fn call(id: &str, name: &str, args: serde_json::Value) -> AssistantContent {
    AssistantContent::tool_call(id, name, args)
}

fn agent(name: &str, model: ScriptedModel) -> SwarmsAgentBuilder<ScriptedModel> {
    SwarmsAgentBuilder::new_with_model(model).agent_name(name)
}

fn is_valid_tool_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[tokio::test]
async fn sub_agent_runs_and_parent_keeps_control() {
    let sub_model = ScriptedModel::new(vec![], "sub answer: 42");
    let sub = agent("Math Helper", sub_model.clone())
        .description("Does arithmetic")
        .build();

    let parent_model = ScriptedModel::new(
        vec![Ok(vec![call(
            "1",
            "delegate_to_Math_Helper",
            json!({"task": "compute 6 * 7"}),
        )])],
        "final answer uses 42",
    );
    let parent = agent("Coordinator", parent_model.clone())
        .add_sub_agent(sub)
        .max_loops(2)
        .build();

    let output = parent.run("What is 6 * 7?".to_string()).await.unwrap();

    assert!(parent_model.tools.lock().unwrap()[0].contains(&"delegate_to_Math_Helper".to_string()));
    assert_eq!(sub_model.calls(), 1);
    assert!(sub_model.requests.lock().unwrap()[0].contains("compute 6 * 7"));
    // The sub-agent's answer comes back as the tool result, and the parent carries on.
    assert_eq!(parent_model.calls(), 2);
    assert!(output.contains("sub answer: 42"), "{output}");
    assert!(output.contains("final answer uses 42"), "{output}");
}

#[tokio::test]
async fn failing_sub_agent_is_reported_to_the_model() {
    let sub_model = ScriptedModel::new(vec![Err("upstream down".into())], "unused");
    let sub = agent("Flaky", sub_model).retry_attempts(1).build();
    let parent_model = ScriptedModel::new(
        vec![Ok(vec![call(
            "1",
            "delegate_to_Flaky",
            json!({"task": "try it"}),
        )])],
        "done",
    );
    let parent = agent("Coordinator", parent_model)
        .add_sub_agent(sub)
        .build();

    let output = parent.run("go".to_string()).await.unwrap();
    assert!(output.contains("sub-agent 'Flaky' failed"), "{output}");
    assert!(output.contains("upstream down"), "{output}");
}

#[tokio::test]
async fn handoff_transfers_control_and_context() {
    let billing_model = ScriptedModel::new(vec![], "refund issued for order 7");
    let billing = agent("Billing", billing_model.clone()).build();

    let triage_model = ScriptedModel::new(
        vec![Ok(vec![call(
            "1",
            "transfer_to_Billing",
            json!({"task": "refund order 7", "context": "customer is on the premium plan"}),
        )])],
        "should not be reached",
    );
    let triage = agent("Triage", triage_model.clone())
        .add_handoff(billing)
        .max_loops(5)
        .build();

    let output = triage
        .run("I want my money back for order 7".to_string())
        .await
        .unwrap();

    // Triage stopped after handing off, and its output ends with Billing's answer.
    assert_eq!(triage_model.calls(), 1);
    assert_eq!(billing_model.calls(), 1);
    assert!(output.contains("refund issued for order 7"), "{output}");
    assert!(!output.contains("should not be reached"), "{output}");

    // Billing got the task, the context, and the conversation so far.
    let received = billing_model.requests.lock().unwrap()[0].clone();
    assert!(
        received.contains("taking over a task from the agent 'Triage'"),
        "{received}"
    );
    assert!(received.contains("refund order 7"), "{received}");
    assert!(
        received.contains("customer is on the premium plan"),
        "{received}"
    );
    assert!(
        received.contains("I want my money back for order 7"),
        "{received}"
    );

    // The handoff is recorded with the target's answer as its result.
    let conversation = triage
        .conversation("I want my money back for order 7")
        .unwrap();
    let handoff = conversation
        .tool_outputs()
        .find(|o| o.name == "transfer_to_Billing")
        .unwrap();
    assert!(handoff.result.contains("refund issued for order 7"));
}

#[tokio::test]
async fn only_the_first_handoff_in_a_turn_runs() {
    let first_model = ScriptedModel::new(vec![], "first handled it");
    let second_model = ScriptedModel::new(vec![], "second handled it");
    let router_model = ScriptedModel::new(
        vec![Ok(vec![
            call("1", "transfer_to_First", json!({"task": "a"})),
            call("2", "transfer_to_Second", json!({"task": "b"})),
        ])],
        "unused",
    );
    let router = agent("Router", router_model)
        .handoffs(vec![
            Box::new(agent("First", first_model.clone()).build()),
            Box::new(agent("Second", second_model.clone()).build()),
        ])
        .build();

    let output = router.run("route me".to_string()).await.unwrap();
    assert_eq!(first_model.calls(), 1);
    assert_eq!(second_model.calls(), 0);
    assert!(output.contains("first handled it"), "{output}");
    assert!(
        output.contains("Not run: only one handoff runs per turn"),
        "{output}"
    );
}

#[tokio::test]
async fn failed_handoff_keeps_control() {
    let target_model = ScriptedModel::new(vec![Err("target broken".into())], "unused");
    let target = agent("Specialist", target_model).retry_attempts(1).build();
    let parent_model = ScriptedModel::new(
        vec![Ok(vec![call(
            "1",
            "transfer_to_Specialist",
            json!({"task": "do it"}),
        )])],
        "handled it myself",
    );
    let parent = agent("Generalist", parent_model.clone())
        .add_handoff(target)
        .max_loops(2)
        .build();

    let output = parent.run("task".to_string()).await.unwrap();
    assert_eq!(parent_model.calls(), 2);
    assert!(
        output.contains("Handoff to 'Specialist' failed"),
        "{output}"
    );
    assert!(output.contains("handled it myself"), "{output}");
}

#[tokio::test]
async fn an_agent_is_never_offered_a_handoff_to_itself() {
    let model = ScriptedModel::new(vec![], "done");
    let twin = agent("Router", ScriptedModel::new(vec![], "twin")).build();
    let router = agent("Router", model.clone()).add_handoff(twin).build();

    router.run("hi".to_string()).await.unwrap();
    let offered = &model.tools.lock().unwrap()[0];
    assert!(
        offered.iter().all(|name| !name.starts_with("transfer_to_")),
        "{offered:?}"
    );
}

#[tokio::test]
async fn delegation_tool_names_are_valid_and_unique() {
    let model = ScriptedModel::new(vec![], "done");
    let sub = |name: &str| agent(name, ScriptedModel::new(vec![], "x")).build();
    let parent = agent("Parent", model.clone())
        .add_sub_agent(sub("Research Agent v2.1"))
        .add_sub_agent(sub("Research Agent v2.1"))
        .add_sub_agent(sub(&"very long name ".repeat(10)))
        .add_sub_agent(sub("日本語"))
        .add_handoff(sub("Research Agent v2.1"))
        .build();

    parent.run("hi".to_string()).await.unwrap();
    let offered = model.tools.lock().unwrap()[0].clone();
    for expected in [
        "delegate_to_Research_Agent_v2_1",
        "delegate_to_Research_Agent_v2_1_2",
        "delegate_to_agent",
        "transfer_to_Research_Agent_v2_1",
    ] {
        assert!(
            offered.contains(&expected.to_string()),
            "{expected} not in {offered:?}"
        );
    }
    assert!(
        offered.iter().all(|name| is_valid_tool_name(name)),
        "{offered:?}"
    );
    let mut unique = offered.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), offered.len(), "{offered:?}");
}

#[tool(description = "Add two numbers")]
fn add_numbers(a: i64, b: i64) -> Result<i64, TestError> {
    Ok(a + b)
}

#[tokio::test]
async fn tool_outputs_are_kept_typed_in_the_conversation() {
    let model = ScriptedModel::new(
        vec![Ok(vec![call("1", "add_numbers", json!({"a": 2, "b": 3}))])],
        "done",
    );
    let agent: SwarmsAgent<ScriptedModel> = agent("Calc", model).add_tool(AddNumbers).build();

    let output = agent.run("What is 2 + 3?".to_string()).await.unwrap();
    let conversation = agent.conversation("What is 2 + 3?").unwrap();
    let outputs: Vec<&ToolCallOutput> = conversation.tool_outputs().collect();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "add_numbers");
    assert_eq!(outputs[0].result_as::<i64>().unwrap(), 5);

    // The text the caller gets back is unchanged.
    assert!(
        output.contains(
            "[Tool name]: add_numbers\n[Tool args]: {\"a\":2,\"b\":3}\n[Tool result]: 5\n\n"
        ),
        "{output}"
    );
    assert!(agent.conversation("never ran").is_none());
}

/// The part after the `Timestamp(millis): N ` line.
fn after_timestamp(content: &Content) -> String {
    let text = content.to_string();
    let (first, rest) = text.split_once('\n').unwrap();
    assert!(first.starts_with("Timestamp(millis): "), "{first}");
    rest.to_string()
}

#[tokio::test]
async fn tool_call_turns_render_the_same_text_as_before() {
    let outputs = vec![
        ToolCallOutput {
            name: "search".into(),
            args: r#"{"q":"rust"}"#.into(),
            result: r#""found it""#.into(),
        },
        ToolCallOutput {
            name: "count".into(),
            args: "{}".into(),
            result: "3".into(),
        },
    ];
    let formatted: String = outputs.iter().map(|o| o.to_string()).collect();
    assert_eq!(
        formatted,
        "[Tool name]: search\n[Tool args]: {\"q\":\"rust\"}\n[Tool result]: \"found it\"\n\n\
         [Tool name]: count\n[Tool args]: {}\n[Tool result]: 3\n\n"
    );

    let role = Role::Assistant("Agent".into());
    let mut typed = AgentConversation::new("Agent".into());
    typed.add_tool_calls(role.clone(), None, outputs.clone());
    let mut plain = AgentConversation::new("Agent".into());
    plain.add(role, formatted);

    // Display, and the history sent to the LLM, match what plain text storage produced.
    assert_eq!(
        after_timestamp(&typed.history[0].content),
        after_timestamp(&plain.history[0].content)
    );
    let typed_llm: Vec<Message> = (&typed).into();
    let plain_llm: Vec<Message> = (&plain).into();
    let strip = |m: &Message| {
        let text = message_text(m);
        text.split_once('\n').unwrap().1.to_string()
    };
    assert_eq!(strip(&typed_llm[0]), strip(&plain_llm[0]));

    // JSON keeps the structure.
    let json = typed.to_json().unwrap();
    let restored: Vec<swarms_rs::structs::conversation::Message> =
        serde_json::from_str(&json).unwrap();
    assert_eq!(restored[0].content, typed.history[0].content);
    assert_eq!(restored[0].content.tool_outputs(), outputs.as_slice());

    // The text export keeps the text (as a text message).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("conversation.txt");
    typed.export_to_file(&path).await.unwrap();
    let mut imported = AgentConversation::new("Agent".into());
    imported.import_from_file(&path).await.unwrap();
    assert_eq!(imported.to_string(), typed.to_string());
}

#[tokio::test]
async fn text_sent_with_task_evaluator_is_kept_as_the_answer() {
    let model = ScriptedModel::new(
        vec![Ok(vec![
            AssistantContent::text("FINAL ANSWER: 42"),
            call("1", "task_evaluator", json!({"status": "Complete"})),
        ])],
        "should not be reached",
    );
    let agent = agent("Solver", model.clone()).max_loops(3).build();

    let output = agent.run("answer".to_string()).await.unwrap();
    assert_eq!(model.calls(), 1);
    assert!(output.contains("FINAL ANSWER: 42"), "{output}");
    assert!(!output.contains("should not be reached"), "{output}");

    // The text comes before the tool lines in the same assistant turn.
    let conversation = agent.conversation("answer").unwrap();
    let turn = conversation.history.last().unwrap().content.to_string();
    let answer_at = turn.find("FINAL ANSWER: 42").unwrap();
    let tool_at = turn.find("[Tool name]: task_evaluator").unwrap();
    assert!(answer_at < tool_at, "{turn}");
    assert_eq!(conversation.tool_outputs().count(), 1);
}

#[tokio::test]
async fn text_sent_with_tool_calls_counts_for_stop_words() {
    let model = ScriptedModel::new(
        vec![Ok(vec![
            AssistantContent::text("All done <DONE>"),
            call("1", "add_numbers", json!({"a": 1, "b": 1})),
        ])],
        "should not be reached",
    );
    let agent = agent("Stopper", model.clone())
        .add_tool(AddNumbers)
        .add_stop_word("<DONE>")
        .max_loops(3)
        .build();

    let output = agent.run("stop early".to_string()).await.unwrap();
    assert_eq!(model.calls(), 1);
    assert!(output.contains("All done <DONE>"), "{output}");
}

#[tokio::test]
async fn text_sent_with_a_handoff_reaches_the_target() {
    let target_model = ScriptedModel::new(vec![], "target answer");
    let target = agent("Target", target_model.clone()).build();
    let source_model = ScriptedModel::new(
        vec![Ok(vec![
            AssistantContent::text("I found the order id: 991"),
            call("1", "transfer_to_Target", json!({"task": "finish"})),
        ])],
        "unused",
    );
    let source = agent("Source", source_model).add_handoff(target).build();

    let output = source.run("help".to_string()).await.unwrap();
    assert!(output.contains("I found the order id: 991"), "{output}");
    assert!(output.contains("target answer"), "{output}");
    let received = target_model.requests.lock().unwrap()[0].clone();
    assert!(received.contains("I found the order id: 991"), "{received}");
}

#[test]
fn tool_call_turn_with_text_round_trips() {
    let mut conversation = AgentConversation::new("Agent".into());
    conversation.add_tool_calls(
        Role::Assistant("Agent".into()),
        Some("Let me check.".into()),
        vec![ToolCallOutput {
            name: "t".into(),
            args: "{}".into(),
            result: "1".into(),
        }],
    );
    let text = conversation.history[0].content.to_string();
    assert!(text.contains("Let me check.\n\n[Tool name]: t\n"), "{text}");

    let json = conversation.to_json().unwrap();
    let restored: Vec<swarms_rs::structs::conversation::Message> =
        serde_json::from_str(&json).unwrap();
    assert_eq!(restored[0].content, conversation.history[0].content);
}

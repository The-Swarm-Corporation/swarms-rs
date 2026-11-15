//! Tests for the `budget` feature: an agent charges each completion's reported usage and
//! stops at its spending cap. Driven by a scripted model, so no API key or network is needed.
#![cfg(feature = "budget")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::future::BoxFuture;
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::completion::AssistantContent;
use swarms_rs::llm::request::{CompletionRequest, CompletionResponse};
use swarms_rs::llm::{CompletionError, Model};
use swarms_rs::llm_budget::{FleetAllocator, ModelTier, Usage};
use swarms_rs::structs::agent::{Agent, AgentError};

/// Always answers "working on it" and reports the same usage for `model`.
#[derive(Clone)]
struct MeteredModel {
    model: String,
    usage: Option<Usage>,
    calls: Arc<AtomicUsize>,
}

impl MeteredModel {
    fn new(model: &str, usage: Option<Usage>) -> Self {
        Self {
            model: model.to_string(),
            usage,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Model for MeteredModel {
    type RawCompletionResponse = Option<Usage>;

    fn completion(
        &self,
        _request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<Option<Usage>>, CompletionError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CompletionResponse {
                choice: vec![AssistantContent::text("working on it")],
                raw_response: self.usage,
            })
        })
    }

    fn usage(&self, response: &Option<Usage>) -> Option<(String, Usage)> {
        response.map(|usage| (self.model.clone(), usage))
    }
}

fn agent(model: MeteredModel) -> SwarmsAgentBuilder<MeteredModel> {
    SwarmsAgentBuilder::new_with_model(model)
        .agent_name("Metered")
        .disable_task_complete_tool()
        .max_loops(50)
}

#[tokio::test]
async fn stops_the_loop_at_the_cap() {
    // gpt-4o: $2.50 per million input tokens, $10 per million output tokens,
    // so each call costs $0.0025 + $0.01 = $0.0125.
    let model = MeteredModel::new("gpt-4o", Some(Usage::new(1_000, 1_000)));
    let agent = agent(model.clone()).budget_usd(0.05).build();

    agent.run("Keep going".to_string()).await.unwrap();

    assert_eq!(
        model.calls(),
        4,
        "four calls reach $0.05, then the loop stops"
    );
    let budget = agent.budget().unwrap();
    assert!((budget.spent_usd() - 0.05).abs() < 1e-9);
    assert!(budget.is_exhausted());

    // Once the cap is spent, no further call is made.
    let error = agent.prompt("One more").await.unwrap_err();
    assert!(matches!(error, AgentError::BudgetError(_)), "{error}");
    assert_eq!(model.calls(), 4);
}

#[tokio::test]
async fn without_a_budget_runs_every_loop() {
    let model = MeteredModel::new("gpt-4o", Some(Usage::new(1_000, 1_000)));
    let agent = agent(model.clone()).max_loops(6).build();

    agent.run("Keep going".to_string()).await.unwrap();

    assert_eq!(model.calls(), 6);
    assert!(agent.budget().is_none());
}

#[tokio::test]
async fn unpriced_models_are_charged_the_standard_tier() {
    let model = MeteredModel::new("in-house-model", Some(Usage::new(1_000, 1_000)));
    let agent = agent(model.clone()).budget_usd(1.0).max_loops(2).build();

    agent.run("Keep going".to_string()).await.unwrap();

    let expected = 2.0 * ModelTier::Standard.estimate_cost(1_000, 1_000);
    assert!((agent.budget().unwrap().spent_usd() - expected).abs() < 1e-9);
}

#[tokio::test]
async fn fleet_agents_share_one_total() {
    let fleet = FleetAllocator::new(0.05, ModelTier::Standard, 0.2).unwrap();
    fleet.register_agent("Researcher", 0.025).unwrap();
    fleet.register_agent("Writer", 0.025).unwrap();

    let researcher_model = MeteredModel::new("gpt-4o", Some(Usage::new(1_000, 1_000)));
    let writer_model = MeteredModel::new("gpt-4o", Some(Usage::new(1_000, 1_000)));
    let researcher = agent(researcher_model.clone())
        .agent_name("Researcher")
        .budget(fleet.get_agent("Researcher").unwrap())
        .build();
    let writer = agent(writer_model.clone())
        .agent_name("Writer")
        .budget(fleet.get_agent("Writer").unwrap())
        .build();

    researcher.run("Research".to_string()).await.unwrap();
    writer.run("Write".to_string()).await.unwrap();

    // $0.025 each is two $0.0125 calls.
    assert_eq!(researcher_model.calls(), 2);
    assert_eq!(writer_model.calls(), 2);
}

#[tokio::test]
async fn a_killed_budget_stops_the_agent_before_any_call() {
    let model = MeteredModel::new("gpt-4o", Some(Usage::new(1_000, 1_000)));
    let agent = agent(model.clone()).budget_usd(1.0).build();
    agent.budget().unwrap().kill("operator stop");

    let error = agent.run("Keep going".to_string()).await.unwrap_err();

    assert!(matches!(error, AgentError::BudgetError(_)), "{error}");
    assert_eq!(model.calls(), 0);
}

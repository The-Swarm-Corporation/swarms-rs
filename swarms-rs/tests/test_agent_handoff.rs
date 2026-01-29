use futures::future::BoxFuture;
use swarms_rs::structs::agent::{Agent, AgentError};
use swarms_rs::structs::conversation::{AgentShortMemory, Role};

#[derive(Clone)]
struct MockTransferAgent {
    name: String,
    id: String,
    pub short_memory: AgentShortMemory,
}

impl MockTransferAgent {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            id: format!("mock-{}", name),
            short_memory: AgentShortMemory::new(),
        }
    }
}

impl Agent for MockTransferAgent {
    fn run(&self, _task: String) -> BoxFuture<'_, Result<String, AgentError>> {
        Box::pin(async move { Ok("ok".to_string()) })
    }

    fn run_multiple_tasks(
        &mut self,
        _tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        Box::pin(async move { Ok(vec![]) })
    }

    fn plan(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn save_task_state(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn is_response_complete(&self, _response: String) -> bool {
        true
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn description(&self) -> String {
        format!("MockTransferAgent: {}", self.name)
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }

    // Provide custom export/import for the test agent
    fn export_handoff(&self, task: String) -> BoxFuture<Result<swarms_rs::structs::agent::AgentHandoff, AgentError>> {
        let conv = self.short_memory.0.get(&task);
        if let Some(conv) = conv {
            let handoff = swarms_rs::structs::agent::AgentHandoff {
                task: task.clone(),
                conversation: conv.clone(),
                from_agent_id: self.id.clone(),
                from_agent_name: self.name.clone(),
            };
            Box::pin(async move { Ok(handoff) })
        } else {
            Box::pin(async move { Err(AgentError::TaskNotFound(task)) })
        }
    }

    fn import_handoff(&self, handoff: swarms_rs::structs::agent::AgentHandoff) -> BoxFuture<Result<(), AgentError>> {
        self.short_memory.0.insert(handoff.task.clone(), handoff.conversation);
        Box::pin(async move { Ok(()) })
    }
}

#[tokio::test]
async fn test_agent_handoff_export_import() {
    let mut agent_a = MockTransferAgent::new("A");

    // populate short memory for a task
    agent_a.short_memory.add(
        "task1",
        "A",
        Role::Assistant("A".to_string()),
        "intermediate result",
    );

    // export handoff
    let handoff = agent_a.export_handoff("task1".to_string()).await.unwrap();

    // import into another agent
    let agent_b = MockTransferAgent::new("B");
    agent_b.import_handoff(handoff).await.unwrap();

    // ensure agent_b now has the task
    let conv = agent_b.short_memory.0.get("task1");
    assert!(conv.is_some());
}

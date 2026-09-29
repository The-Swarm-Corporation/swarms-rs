mod test_utils;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dashmap::DashMap;
    use futures::future;

    use swarms_rs::structs::graph_workflow::{DAGWorkflow, Flow, GraphWorkflowError};

    // Import test utilities from the tests module
    use crate::test_utils::{MockAgent, create_failing_agent, create_mock_agent};

    #[test]
    fn test_dag_creation() {
        let workflow = DAGWorkflow::new("test", "Test workflow");
        assert_eq!(workflow.name, "test");
        assert_eq!(workflow.description, "Test workflow");
        assert_eq!(workflow.agents_len(), 0);
        assert_eq!(workflow.node_count(), 0);
        assert_eq!(workflow.edge_count(), 0);
    }

    #[test]
    fn test_agent_registration() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "Test agent", "response1"));

        assert_eq!(workflow.agents_len(), 1);
        assert_eq!(workflow.node_count(), 1);
        assert!(workflow.contains_agent_name("agent1"));
    }

    #[test]
    fn test_agent_connection() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        let result = workflow.connect_agents("agent1", "agent2", Flow::default());
        assert!(result.is_ok());
        assert_eq!(workflow.edge_count(), 1);
    }

    #[test]
    fn test_agent_connection_failure_nonexistent_agent() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "Test agent", "response1"));

        let result = workflow.connect_agents("agent1", "nonexistent", Flow::default());
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));

        let result = workflow.connect_agents("nonexistent", "agent1", Flow::default());
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));
    }

    #[test]
    fn test_cycle_detection() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));
        workflow.register_agent(create_mock_agent("3", "agent3", "Third agent", "response3"));

        // agent1 -> agent2 -> agent3
        let result1 = workflow.connect_agents("agent1", "agent2", Flow::default());
        assert!(result1.is_ok());
        let result2 = workflow.connect_agents("agent2", "agent3", Flow::default());
        assert!(result2.is_ok());

        // cycle it: agent3 -> agent1
        let result3 = workflow.connect_agents("agent3", "agent1", Flow::default());
        assert!(matches!(result3, Err(GraphWorkflowError::CycleDetected)));

        // edge should not be added
        assert_eq!(workflow.edge_count(), 2);
    }

    #[test]
    fn test_agent_disconnection() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();
        assert_eq!(workflow.edge_count(), 1);

        let result = workflow.disconnect_agents("agent1", "agent2");
        assert!(result.is_ok());
        assert_eq!(workflow.edge_count(), 0);
    }

    #[test]
    fn test_agent_disconnection_failure() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        // try to disconnect non-existent edge
        let result = workflow.disconnect_agents("agent1", "agent2");
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));

        // try to disconnect non-existent agent
        let result = workflow.disconnect_agents("nonexistent", "agent2");
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));
    }

    #[test]
    fn test_agent_removal() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();
        assert_eq!(workflow.agents_len(), 2);
        assert_eq!(workflow.node_count(), 2);

        let result = workflow.remove_agent("agent1");
        assert!(result.is_ok());
        assert_eq!(workflow.agents_len(), 1);
        assert_eq!(workflow.node_count(), 1);
        assert!(!workflow.contains_agent_name("agent1"));

        assert_eq!(workflow.edge_count(), 0);
    }

    #[test]
    fn test_agent_removal_nonexistent() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");

        let result = workflow.remove_agent("nonexistent");
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));
    }

    #[test]
    fn test_connect_after_agent_removal() {
        // Removing a node leaves a gap in the graph's indices; the cycle check must cope
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));
        workflow.register_agent(create_mock_agent("3", "agent3", "Third agent", "response3"));

        workflow.remove_agent("agent1").unwrap();
        workflow
            .connect_agents("agent2", "agent3", Flow::default())
            .unwrap();
        assert!(matches!(
            workflow.connect_agents("agent3", "agent2", Flow::default()),
            Err(GraphWorkflowError::CycleDetected)
        ));
    }

    /// agent1 -> agent2 -> agent4 and agent1 -> agent3 -> agent4
    fn diamond(agent2: Box<MockAgent>, to_agent3: Flow) -> DAGWorkflow {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(agent2);
        workflow.register_agent(create_mock_agent("3", "agent3", "Third agent", "response3"));
        workflow.register_agent(create_mock_agent("4", "agent4", "Join agent", "response4"));
        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();
        workflow
            .connect_agents("agent1", "agent3", to_agent3)
            .unwrap();
        workflow
            .connect_agents("agent2", "agent4", Flow::default())
            .unwrap();
        workflow
            .connect_agents("agent3", "agent4", Flow::default())
            .unwrap();
        workflow
    }

    #[tokio::test]
    async fn test_join_runs_when_a_branch_condition_is_false() {
        let never = Flow {
            transform: None,
            condition: Some(Arc::new(|_: &str| false)),
        };
        let mut workflow = diamond(
            create_mock_agent("2", "agent2", "Second agent", "response2"),
            never,
        );

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert!(!results.contains_key("agent3"));
        assert_eq!(
            results.get("agent4").unwrap().value().as_deref().unwrap(),
            "response4"
        );
    }

    #[tokio::test]
    async fn test_join_runs_when_a_parent_fails() {
        let mut workflow = diamond(
            create_failing_agent("2", "agent2", "fail error"),
            Flow::default(),
        );

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert!(results.get("agent2").unwrap().is_err());
        assert!(results.get("agent4").unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_join_ignores_parent_unreachable_from_start() {
        // agent0 -> agent2 <- agent1, starting from agent1: agent0 never runs
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("0", "agent0", "Other root", "response0"));
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent("2", "agent2", "Join agent", "response2"));
        workflow
            .connect_agents("agent0", "agent2", Flow::default())
            .unwrap();
        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert!(!results.contains_key("agent0"));
        assert!(results.get("agent2").unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_execute_single_agent() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "Test agent", "response1"));

        let result = workflow.execute_agent("agent1", "input".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "response1");
    }

    #[tokio::test]
    async fn test_execute_single_agent_failure() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_failing_agent("1", "agent1", "test error"));

        let result = workflow.execute_agent("agent1", "input".to_string()).await;
        assert!(matches!(result, Err(GraphWorkflowError::AgentError(_))));
    }

    #[tokio::test]
    async fn test_execute_single_agent_not_found() {
        let workflow = DAGWorkflow::new("test", "Test workflow");

        let result = workflow
            .execute_agent("nonexistent", "input".to_string())
            .await;
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn test_execute_workflow_linear() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results.get("agent1").unwrap().as_ref().unwrap(),
            "response1"
        );
        assert_eq!(
            results.get("agent2").unwrap().as_ref().unwrap(),
            "response2"
        );
    }

    #[tokio::test]
    async fn test_execute_workflow_branching() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "Root agent", "response1"));
        workflow.register_agent(create_mock_agent("2", "agent2", "Branch 1", "response2"));
        workflow.register_agent(create_mock_agent("3", "agent3", "Branch 2", "response3"));

        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();
        workflow
            .connect_agents("agent1", "agent3", Flow::default())
            .unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(
            results.get("agent1").unwrap().as_ref().unwrap(),
            "response1"
        );
        assert_eq!(
            results.get("agent2").unwrap().as_ref().unwrap(),
            "response2"
        );
        assert_eq!(
            results.get("agent3").unwrap().as_ref().unwrap(),
            "response3"
        );
    }

    #[tokio::test]
    async fn test_execute_workflow_with_transformation() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        let transform_fn = Arc::new(|input: String| format!("transformed: {}", input));
        let flow = Flow {
            transform: Some(transform_fn),
            condition: None,
        };

        workflow.connect_agents("agent1", "agent2", flow).unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert_eq!(results.len(), 2);

        let structure = workflow.get_workflow_structure();
        let agent1_connections = &structure["agent1"];
        assert_eq!(agent1_connections.len(), 1);
        assert_eq!(agent1_connections[0].0, "agent2");
        assert_eq!(agent1_connections[0].1, Some("transform".to_string()));
    }

    #[tokio::test]
    async fn test_execute_workflow_with_condition_true() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "true"));
        workflow.register_agent(create_mock_agent("2", "agent2", "Second agent", "executed"));

        let true_condition = Arc::new(|output: &str| output.contains("true"));

        workflow
            .connect_agents(
                "agent1",
                "agent2",
                Flow {
                    transform: None,
                    condition: Some(true_condition),
                },
            )
            .unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains_key("agent1"));
        assert!(results.contains_key("agent2"));
    }

    #[tokio::test]
    async fn test_execute_workflow_with_condition_false() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "not executed",
        ));

        let false_condition = Arc::new(|output: &str| output.contains("nonexistent"));

        workflow
            .connect_agents(
                "agent1",
                "agent2",
                Flow {
                    transform: None,
                    condition: Some(false_condition),
                },
            )
            .unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results.contains_key("agent1"));
        assert!(!results.contains_key("agent2"));
    }

    #[tokio::test]
    async fn test_workflow_execution_start_agent_not_found() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));

        let result = workflow.execute_workflow("nonexistent", "input").await;
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));
    }

    #[tokio::test]
    async fn test_workflow_execution_with_failing_agent() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "agent1", "First agent", "response1"));
        workflow.register_agent(create_failing_agent("2", "agent2", "fail error"));
        workflow.register_agent(create_mock_agent("3", "agent3", "Third agent", "response3"));

        // agent1 -> agent2 -> agent3
        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();
        workflow
            .connect_agents("agent2", "agent3", Flow::default())
            .unwrap();

        let results = workflow.execute_workflow("agent1", "input").await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains_key("agent1"));
        assert!(results.contains_key("agent2"));
        assert!(!results.contains_key("agent3"));

        let agent2_result = results.get("agent2").unwrap();
        assert!(agent2_result.is_err());
    }

    #[test]
    fn test_find_execution_paths() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("0", "start", "Starting point", "start"));
        workflow.register_agent(create_mock_agent("1", "a", "Path A", "a"));
        workflow.register_agent(create_mock_agent("2", "b", "Path B", "b"));
        workflow.register_agent(create_mock_agent("3", "c", "End of A", "c"));
        workflow.register_agent(create_mock_agent("4", "d", "End of B", "d"));

        workflow
            .connect_agents("start", "a", Flow::default())
            .unwrap();
        workflow
            .connect_agents("start", "b", Flow::default())
            .unwrap();
        workflow.connect_agents("a", "c", Flow::default()).unwrap();
        workflow.connect_agents("b", "d", Flow::default()).unwrap();

        let paths = workflow.find_execution_paths("start").unwrap();
        assert_eq!(paths.len(), 2);

        // path should be [start, a, c] or [start, b, d]
        let has_path1 = paths
            .iter()
            .any(|p| p == &vec!["start".to_string(), "a".to_string(), "c".to_string()]);
        let has_path2 = paths
            .iter()
            .any(|p| p == &vec!["start".to_string(), "b".to_string(), "d".to_string()]);

        assert!(has_path1);
        assert!(has_path2);
    }

    #[test]
    fn test_find_execution_paths_start_agent_not_found() {
        let workflow = DAGWorkflow::new("test", "Test workflow");

        let result = workflow.find_execution_paths("nonexistent");
        assert!(matches!(result, Err(GraphWorkflowError::AgentNotFound(_))));
    }

    #[test]
    fn test_find_execution_paths_diamond_pattern() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("0", "start", "Start", "start"));
        workflow.register_agent(create_mock_agent("1", "a", "Middle A", "a"));
        workflow.register_agent(create_mock_agent("2", "b", "Middle B", "b"));
        workflow.register_agent(create_mock_agent("3", "end", "End", "end"));

        //            start -> a -> end
        //                 \-> b -/
        workflow
            .connect_agents("start", "a", Flow::default())
            .unwrap();
        workflow
            .connect_agents("start", "b", Flow::default())
            .unwrap();
        workflow
            .connect_agents("a", "end", Flow::default())
            .unwrap();
        workflow
            .connect_agents("b", "end", Flow::default())
            .unwrap();

        let paths = workflow.find_execution_paths("start").unwrap();
        assert_eq!(paths.len(), 2);

        // path should be [start, a, end] or [start, b, end]
        let has_path1 = paths
            .iter()
            .any(|p| p == &vec!["start".to_string(), "a".to_string(), "end".to_string()]);
        let has_path2 = paths
            .iter()
            .any(|p| p == &vec!["start".to_string(), "b".to_string(), "end".to_string()]);

        assert!(has_path1);
        assert!(has_path2);
    }

    #[test]
    fn test_detect_potential_deadlocks() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "a", "Agent A", "a"));
        workflow.register_agent(create_mock_agent("2", "b", "Agent B", "b"));
        workflow.register_agent(create_mock_agent("3", "c", "Agent C", "c"));

        // a -> b -> c
        workflow.connect_agents("a", "b", Flow::default()).unwrap();
        workflow.connect_agents("b", "c", Flow::default()).unwrap();

        // no cycles, should return empty vector
        let deadlocks = workflow.detect_potential_deadlocks();
        assert_eq!(deadlocks.len(), 0);

        // try to add c -> a, which should fail because has_cycle prevents it
        let result = workflow.connect_agents("c", "a", Flow::default());
        assert!(matches!(result, Err(GraphWorkflowError::CycleDetected)));
    }

    #[test]
    fn test_get_workflow_structure() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "a", "Agent A", "a"));
        workflow.register_agent(create_mock_agent("2", "b", "Agent B", "b"));
        workflow.register_agent(create_mock_agent("3", "c", "Agent C", "c"));

        workflow.connect_agents("a", "b", Flow::default()).unwrap();

        let transform_fn = Arc::new(|input: String| format!("transformed: {}", input));
        let flow = Flow {
            transform: Some(transform_fn),
            condition: None,
        };

        workflow.connect_agents("b", "c", flow).unwrap();

        let structure = workflow.get_workflow_structure();
        assert_eq!(structure.len(), 3);

        assert_eq!(structure["a"].len(), 1);
        assert_eq!(structure["a"][0].0, "b");
        assert_eq!(structure["a"][0].1, None);

        assert_eq!(structure["b"].len(), 1);
        assert_eq!(structure["b"][0].0, "c");
        assert_eq!(structure["b"][0].1, Some("transform".to_string())); // has transform

        assert_eq!(structure["c"].len(), 0); // c is a leaf node
    }

    #[test]
    fn test_export_workflow_dot() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");
        workflow.register_agent(create_mock_agent("1", "a", "Agent A", "a"));
        workflow.register_agent(create_mock_agent("2", "b", "Agent B", "b"));

        workflow.connect_agents("a", "b", Flow::default()).unwrap();

        let dot = workflow.export_workflow_dot();

        assert!(dot.contains("digraph {"));
        assert!(dot.contains("\"a\" [label=\"a\"]"));
        assert!(dot.contains("\"b\" [label=\"b\"]"));
        assert!(dot.contains("\"a\" -> \"b\""));
        assert!(dot.contains("}"));
    }

    #[tokio::test]
    async fn test_caching_execution_results() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");

        // mock counter agent
        let mut agent = Box::new(MockAgent::new());
        let agent_name = "counter".to_string();
        agent.expect_name().return_const(agent_name.clone());
        agent.expect_id().return_const("1".to_string());
        agent
            .expect_description()
            .return_const("Counter Agent".to_string());

        let mut count = 0;
        agent.expect_run().returning(move |_| {
            count += 1;
            Box::pin(future::ready(Ok(format!("Called {} times", count))))
        });

        agent.expect_is_response_complete().returning(|_| true);
        agent
            .expect_run_multiple_tasks()
            .returning(|_| Box::pin(future::ready(Ok(vec![]))));
        agent
            .expect_plan()
            .returning(|_| Box::pin(future::ready(Ok(()))));
        agent
            .expect_query_long_term_memory()
            .returning(|_| Box::pin(future::ready(Ok(()))));
        agent
            .expect_save_task_state()
            .returning(|_| Box::pin(future::ready(Ok(()))));

        workflow.register_agent(agent);

        // first execution
        let results1 = workflow
            .execute_workflow("counter", "input1")
            .await
            .unwrap();
        assert_eq!(
            results1.get("counter").unwrap().as_ref().unwrap(),
            "Called 1 times"
        );

        // second execution (should reset and call again)
        let results2 = workflow
            .execute_workflow("counter", "input2")
            .await
            .unwrap();
        assert_eq!(
            results2.get("counter").unwrap().as_ref().unwrap(),
            "Called 2 times"
        );

        // call execute_agent directly (should not use cache)
        let result3 = workflow
            .execute_agent("counter", "input3".to_string())
            .await
            .unwrap();
        assert_eq!(result3, "Called 3 times");
    }

    #[tokio::test]
    async fn test_execute_node_result_caching() {
        let mut workflow = DAGWorkflow::new("test", "Test workflow");

        // Create a mock agent that records the number of calls
        let mut agent1 = Box::new(MockAgent::new());
        agent1.expect_name().return_const("agent1".to_string());
        agent1.expect_id().return_const("1".to_string());
        agent1
            .expect_description()
            .return_const("First agent".to_string());

        // Set a counter to verify that the run method was called only once
        let mut run_count = 0;
        agent1.expect_run().returning(move |input| {
            run_count += 1;
            Box::pin(future::ready(Ok(format!(
                "response for '{}' (call #{})",
                input, run_count
            ))))
        });

        agent1.expect_is_response_complete().returning(|_| true);

        agent1
            .expect_run_multiple_tasks()
            .returning(|_| Box::pin(future::ready(Ok(vec![]))));
        agent1
            .expect_plan()
            .returning(|_| Box::pin(future::ready(Ok(()))));
        agent1
            .expect_query_long_term_memory()
            .returning(|_| Box::pin(future::ready(Ok(()))));
        agent1
            .expect_save_task_state()
            .returning(|_| Box::pin(future::ready(Ok(()))));

        workflow.register_agent(agent1);

        // Create a normal second proxy
        workflow.register_agent(create_mock_agent(
            "2",
            "agent2",
            "Second agent",
            "response2",
        ));

        // connect the two agents
        workflow
            .connect_agents("agent1", "agent2", Flow::default())
            .unwrap();

        let agent1_idx = workflow.get_node_index("agent1").unwrap();

        // create shared data structures
        let results = Arc::new(DashMap::new());
        let edge_tracker = Arc::new(DashMap::new());
        let processed_nodes = Arc::new(DashMap::new());

        // first execution of agent1
        let result1 = workflow
            .execute_node(
                agent1_idx,
                "input1".to_string(),
                Arc::clone(&results),
                Arc::clone(&edge_tracker),
                Arc::clone(&processed_nodes),
            )
            .await
            .unwrap();

        assert_eq!(result1, "response for 'input1' (call #1)");
        assert!(results.contains_key("agent1"));
        assert!(results.contains_key("agent2")); // agent2 also executed

        // second execution of agent1 with a different input
        let result2 = workflow
            .execute_node(
                agent1_idx,
                "input2".to_string(),
                Arc::clone(&results),
                Arc::clone(&edge_tracker),
                Arc::clone(&processed_nodes),
            )
            .await
            .unwrap();

        // the results should be the same, indicating that the agent was not executed again
        assert_eq!(result2, "response for 'input1' (call #1)"); // not "response for 'input2' (call #1)"

        // clear the results map
        results.clear();

        // third execution of agent1
        let result3 = workflow
            .execute_node(
                agent1_idx,
                "input3".to_string(),
                Arc::clone(&results),
                Arc::clone(&edge_tracker),
                Arc::clone(&processed_nodes),
            )
            .await
            .unwrap();

        // the results should contain the new call count, indicating that the agent was re-executed
        assert_eq!(result3, "response for 'input3' (call #2)");
    }
}

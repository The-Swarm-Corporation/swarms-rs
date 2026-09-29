use std::{
    collections::{HashMap, HashSet, hash_map},
    sync::Arc,
    time::Duration,
};

use dashmap::{DashMap, DashSet};
use futures::future::BoxFuture;
use petgraph::{
    Direction,
    graph::{EdgeIndex, NodeIndex},
    prelude::StableGraph,
    visit::{Dfs, EdgeRef, IntoEdgeReferences},
};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::structs::agent::Agent;

/// The main graph-based workflow structure
pub struct DAGWorkflow {
    pub name: String,
    pub description: String,
    /// Store all registered agents
    agents: DashMap<String, Box<dyn Agent>>,
    /// The workflow graph
    workflow: StableGraph<AgentNode, Flow>,
    /// Map from agent name to node index for quick lookup
    name_to_node: HashMap<String, NodeIndex>,
}

impl DAGWorkflow {
    pub fn new<S: Into<String>>(name: S, description: S) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            agents: DashMap::new(),
            workflow: StableGraph::new(),
            name_to_node: HashMap::new(),
        }
    }

    /// Get the number of registered agents
    pub fn agents_len(&self) -> usize {
        self.agents.len()
    }

    /// Get the number of nodes in the workflow graph
    pub fn node_count(&self) -> usize {
        self.workflow.node_count()
    }

    /// Get the number of edges in the workflow graph
    pub fn edge_count(&self) -> usize {
        self.workflow.edge_count()
    }

    /// Check if an agent name exists in the name_to_node mapping
    pub fn contains_agent_name(&self, name: &str) -> bool {
        self.name_to_node.contains_key(name)
    }

    /// Get the node index for an agent name (for testing purposes)
    pub fn get_node_index(&self, name: &str) -> Option<NodeIndex> {
        self.name_to_node.get(name).copied()
    }

    /// Register an agent with the orchestrator
    pub fn register_agent(&mut self, agent: Box<dyn Agent>) {
        let agent_name = agent.name();
        self.agents.insert(agent_name.clone(), agent);

        // If agent isn't already in the graph, add it
        if let hash_map::Entry::Vacant(e) = self.name_to_node.entry(agent_name.clone()) {
            let node_idx = self.workflow.add_node(AgentNode {
                name: agent_name.clone(),
                last_result: Mutex::new(None),
            });
            e.insert(node_idx);
        }
    }

    /// Add a flow connection between two agents
    pub fn connect_agents(
        &mut self,
        from: &str,
        to: &str,
        flow: Flow,
    ) -> Result<EdgeIndex, GraphWorkflowError> {
        // Ensure both agents exist
        if !self.agents.contains_key(from) {
            return Err(GraphWorkflowError::AgentNotFound(format!(
                "Source agent '{}' not found",
                from
            )));
        }
        if !self.agents.contains_key(to) {
            return Err(GraphWorkflowError::AgentNotFound(format!(
                "Target agent '{}' not found",
                to
            )));
        }

        // Get node indices, creating nodes if necessary
        let from_entry = self.name_to_node.entry(from.to_string());
        let from_idx = *from_entry.or_insert_with(|| {
            self.workflow.add_node(AgentNode {
                name: from.to_string(),
                last_result: Mutex::new(None),
            })
        });

        let to_entry = self.name_to_node.entry(to.to_string());
        let to_idx = *to_entry.or_insert_with(|| {
            self.workflow.add_node(AgentNode {
                name: to.to_string(),
                last_result: Mutex::new(None),
            })
        });

        // Add the edge
        let edge_idx = self.workflow.add_edge(from_idx, to_idx, flow);

        // Check for cycles
        if self.has_cycle() {
            // Remove the edge we just added
            self.workflow.remove_edge(edge_idx);
            return Err(GraphWorkflowError::CycleDetected);
        }

        Ok(edge_idx)
    }

    /// Check if the workflow has a cycle
    fn has_cycle(&self) -> bool {
        // StableGraph keeps indices stable across removals, so index-sized vectors
        // (node_count) go out of bounds; petgraph's check handles the gaps.
        petgraph::algo::is_cyclic_directed(&self.workflow)
    }

    /// Remove an agent connection
    pub fn disconnect_agents(&mut self, from: &str, to: &str) -> Result<(), GraphWorkflowError> {
        let from_idx = self.name_to_node.get(from).ok_or_else(|| {
            GraphWorkflowError::AgentNotFound(format!("Source agent '{}' not found", from))
        })?;
        let to_idx = self.name_to_node.get(to).ok_or_else(|| {
            GraphWorkflowError::AgentNotFound(format!("Target agent '{}' not found", to))
        })?;

        // Find and remove the edge
        if let Some(edge) = self.workflow.find_edge(*from_idx, *to_idx) {
            self.workflow.remove_edge(edge);
            Ok(())
        } else {
            Err(GraphWorkflowError::AgentNotFound(format!(
                "No connection from '{}' to '{}'",
                from, to
            )))
        }
    }

    /// Remove an agent from the orchestrator
    pub fn remove_agent(&mut self, name: &str) -> Result<(), GraphWorkflowError> {
        if let Some(node_idx) = self.name_to_node.remove(name) {
            self.workflow.remove_node(node_idx);
            self.agents.remove(name);
            Ok(())
        } else {
            Err(GraphWorkflowError::AgentNotFound(format!(
                "Agent '{}' not found",
                name
            )))
        }
    }

    /// Execute a specific agent
    pub async fn execute_agent(
        &self,
        name: &str,
        input: String,
    ) -> Result<String, GraphWorkflowError> {
        if let Some(agent) = self.agents.get(name) {
            agent
                .run(input)
                .await
                .map_err(|e| GraphWorkflowError::AgentError(e.to_string()))
        } else {
            Err(GraphWorkflowError::AgentNotFound(format!(
                "Agent '{}' not found",
                name
            )))
        }
    }

    /// Execute the entire workflow starting from a specific agent
    pub async fn execute_workflow(
        &mut self,
        start_agent: &str,
        input: impl Into<String>,
    ) -> Result<DashMap<String, Result<String, GraphWorkflowError>>, GraphWorkflowError> {
        let input = input.into();

        let start_idx = self.name_to_node.get(start_agent).ok_or_else(|| {
            GraphWorkflowError::AgentNotFound(format!("Start agent '{}' not found", start_agent))
        })?;

        // Reset all results
        let node_idxs = self.workflow.node_indices().collect::<Vec<_>>();
        for idx in node_idxs {
            if let Some(node_weight) = self.workflow.node_weight_mut(idx) {
                let mut last_result = node_weight.last_result.lock().await;
                *last_result = None;
            }
        }

        // Create a shared results map for all agents to write to
        let results = Arc::new(DashMap::new());
        // Create a shared tracking state for the entire workflow
        let edge_tracker = Arc::new(DashMap::new());
        let processed_nodes = Arc::new(DashMap::new());

        // Edges from nodes the start agent can't reach will never fire; resolve them
        // up front as skipped so they don't block a join.
        let mut reachable = HashSet::new();
        let mut dfs = Dfs::new(&self.workflow, *start_idx);
        while let Some(node) = dfs.next(&self.workflow) {
            reachable.insert(node);
        }
        for edge in self.workflow.edge_references() {
            if !reachable.contains(&edge.source()) {
                edge_tracker.insert((edge.source(), edge.target()), false);
            }
        }
        // Execute the workflow
        let started = Arc::new(DashSet::new());
        started.insert(*start_idx);
        self.execute_node_tracked(
            *start_idx,
            input,
            Arc::clone(&results),
            edge_tracker,
            processed_nodes,
            started,
        )
        .await?;
        Ok(Arc::into_inner(results).expect("Results should not be poisoned"))
    }

    pub async fn execute_node(
        &self,
        node_idx: NodeIndex,
        input: String,
        results: Arc<DashMap<String, Result<String, GraphWorkflowError>>>,
        edge_tracker: Arc<DashMap<(NodeIndex, NodeIndex), bool>>,
        processed_nodes: Arc<DashMap<NodeIndex, Vec<(NodeIndex, String)>>>,
    ) -> Result<String, GraphWorkflowError> {
        let started = Arc::new(DashSet::new());
        started.insert(node_idx);
        self.execute_node_tracked(
            node_idx,
            input,
            results,
            edge_tracker,
            processed_nodes,
            started,
        )
        .await
    }

    /// `started` holds every node already claimed in this run, so a node that several
    /// branches make ready at the same moment still runs once.
    async fn execute_node_tracked(
        &self,
        node_idx: NodeIndex,
        input: String,
        results: Arc<DashMap<String, Result<String, GraphWorkflowError>>>,
        edge_tracker: Arc<DashMap<(NodeIndex, NodeIndex), bool>>,
        processed_nodes: Arc<DashMap<NodeIndex, Vec<(NodeIndex, String)>>>,
        started: Arc<DashSet<NodeIndex>>,
    ) -> Result<String, GraphWorkflowError> {
        // Get the agent name from the node
        let agent_name = &self
            .workflow
            .node_weight(node_idx)
            .ok_or_else(|| {
                GraphWorkflowError::AgentNotFound("Node not found in graph".to_string())
            })?
            .name;

        // Check if we already have a result for this node (avoid duplicate work)
        if let Some(entry) = results.get(agent_name) {
            return entry.value().clone();
        }

        // Execute the agent with timeout protection; a timeout is recorded like any other failure
        let result = tokio::time::timeout(
            Duration::from_secs(300), // 5-minute timeout
            self.execute_agent(agent_name, input),
        )
        .await
        .unwrap_or_else(|_| Err(GraphWorkflowError::Timeout(agent_name.clone())));

        // Store the result
        results.entry(agent_name.clone()).or_insert(result.clone());

        // Update the node's last result
        if let Some(node_weight) = self.workflow.node_weight(node_idx) {
            let mut last_result = node_weight.last_result.lock().await;
            *last_result = Some(result.clone());
        }

        if let Err(e) = &result {
            tracing::error!("Agent '{}' execution failed: {:?}", agent_name, e);
        }

        // Hand the output to connected agents, or mark their inputs as skipped on failure
        self.propagate(
            node_idx,
            result.as_ref().ok().map(String::as_str),
            results,
            edge_tracker,
            processed_nodes,
            started,
        )
        .await;

        result
    }

    /// Resolve every outgoing edge of `node_idx`, then run each target whose incoming
    /// edges are all resolved.
    ///
    /// An edge is taken (`true` in `edge_tracker`) when the node produced `output` and the
    /// edge's condition passes; otherwise it is skipped (`false`). A target runs once all
    /// of its incoming edges are resolved and at least one was taken. If none was, the
    /// target is skipped as well and the skip propagates downstream.
    fn propagate<'a>(
        &'a self,
        node_idx: NodeIndex,
        output: Option<&'a str>,
        results: Arc<DashMap<String, Result<String, GraphWorkflowError>>>,
        edge_tracker: Arc<DashMap<(NodeIndex, NodeIndex), bool>>,
        processed_nodes: Arc<DashMap<NodeIndex, Vec<(NodeIndex, String)>>>,
        started: Arc<DashSet<NodeIndex>>,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let mut targets = Vec::new();
            for edge in self.workflow.edges_directed(node_idx, Direction::Outgoing) {
                let target = edge.target();
                let flow = edge.weight();
                // if no condition, always take the edge
                let taken =
                    output.filter(|out| flow.condition.as_ref().is_none_or(|cond| cond(out)));
                if let Some(out) = taken {
                    let next_input = flow
                        .transform
                        .as_ref()
                        .map_or_else(|| out.to_string(), |transform| transform(out.to_string()));
                    processed_nodes
                        .entry(target)
                        .or_default()
                        .push((node_idx, next_input));
                }
                edge_tracker.insert((node_idx, target), taken.is_some());
                // parallel edges to the same target must not start it twice
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }

            let futures = targets.into_iter().map(|target| {
                let results = Arc::clone(&results);
                let edge_tracker = Arc::clone(&edge_tracker);
                let processed_nodes = Arc::clone(&processed_nodes);
                let started = Arc::clone(&started);
                async move {
                    let all_resolved = self
                        .workflow
                        .edges_directed(target, Direction::Incoming)
                        .all(|e| edge_tracker.contains_key(&(e.source(), target)));
                    // Another branch may have resolved the last edge first and already
                    // started (or skipped) this target; claim it atomically.
                    if !all_resolved || !started.insert(target) {
                        return;
                    }

                    let aggregated_input = processed_nodes.get(&target).map(|inputs| {
                        inputs
                            .iter()
                            .map(|(source_idx, input)| {
                                format!("[From {}] {}\n", self.workflow[*source_idx].name, input)
                            })
                            .collect::<String>()
                    });

                    match aggregated_input {
                        Some(input) => {
                            if let Err(e) = self
                                .execute_node_tracked(
                                    target,
                                    input,
                                    results,
                                    edge_tracker,
                                    processed_nodes,
                                    started,
                                )
                                .await
                            {
                                tracing::error!("Failed to execute node: {:?}", e);
                            }
                        },
                        None => {
                            self.propagate(
                                target,
                                None,
                                results,
                                edge_tracker,
                                processed_nodes,
                                started,
                            )
                            .await
                        },
                    }
                }
            });

            // Execute connected agents concurrently
            futures::future::join_all(futures).await;
        })
    }

    /// Get the current workflow as a visualization-friendly format
    pub fn get_workflow_structure(&self) -> HashMap<String, Vec<(String, Option<String>)>> {
        let mut structure = HashMap::new();

        for node_idx in self.workflow.node_indices() {
            if let Some(node) = self.workflow.node_weight(node_idx) {
                let mut connections = Vec::new();

                for edge in self.workflow.edges_directed(node_idx, Direction::Outgoing) {
                    if let Some(target) = self.workflow.node_weight(edge.target()) {
                        // TODO: can add more edge metadata here if needed
                        let edge_label = if edge.weight().transform.is_some() {
                            Some("transform".to_string())
                        } else {
                            None
                        };

                        connections.push((target.name.clone(), edge_label));
                    }
                }

                structure.insert(node.name.clone(), connections);
            }
        }

        structure
    }

    /// Export the workflow to a format that can be visualized (e.g., DOT format for Graphviz)
    pub fn export_workflow_dot(&self) -> String {
        // TODO: can use petgraph's built-in dot
        // let dot = Dot::with_config(&self.workflow, &[dot::Config::EdgeNoLabel]);

        let mut dot = String::from("digraph {\n");

        // Add nodes
        for node_idx in self.workflow.node_indices() {
            if let Some(node) = self.workflow.node_weight(node_idx) {
                dot.push_str(&format!(
                    "    \"{}\" [label=\"{}\"];\n",
                    node.name, node.name
                ));
            }
        }

        // Add edges
        for edge in self.workflow.edge_indices() {
            if let Some((source, target)) = self.workflow.edge_endpoints(edge)
                && let (Some(source_node), Some(target_node)) = (
                    self.workflow.node_weight(source),
                    self.workflow.node_weight(target),
                )
            {
                dot.push_str(&format!(
                    "    \"{}\" -> \"{}\";\n",
                    source_node.name, target_node.name
                ));
            }
        }

        dot.push_str("}\n");
        dot
    }

    /// Helper method to find all possible execution paths
    pub fn find_execution_paths(
        &self,
        start_agent: &str,
    ) -> Result<Vec<Vec<String>>, GraphWorkflowError> {
        let start_idx = self.name_to_node.get(start_agent).ok_or_else(|| {
            GraphWorkflowError::AgentNotFound(format!("Start agent '{}' not found", start_agent))
        })?;

        let mut paths = Vec::new();
        let mut current_path = Vec::new();

        self.dfs_paths(*start_idx, &mut current_path, &mut paths);

        Ok(paths)
    }

    fn dfs_paths(
        &self,
        node_idx: NodeIndex,
        current_path: &mut Vec<String>,
        all_paths: &mut Vec<Vec<String>>,
    ) {
        if let Some(node) = self.workflow.node_weight(node_idx) {
            // Add current node to path
            current_path.push(node.name.clone());

            // Check if this is a leaf node (no outgoing edges)
            let has_outgoing = self
                .workflow
                .neighbors_directed(node_idx, Direction::Outgoing)
                .count()
                > 0;

            if !has_outgoing {
                // We've reached a leaf node, save this path
                all_paths.push(current_path.clone());
            } else {
                // Continue DFS for all neighbors
                for neighbor in self
                    .workflow
                    .neighbors_directed(node_idx, Direction::Outgoing)
                {
                    self.dfs_paths(neighbor, current_path, all_paths);
                }
            }

            // Backtrack
            current_path.pop();
        }
    }

    /// Detect potential deadlocks in the workflow. Whether there will actually be a deadlock depends on the flow at execution time.
    ///
    /// ## Info
    ///
    /// Maybe we need a monitor to detect deadlocks instead of this function.
    ///
    /// ## Returns
    ///
    /// Returns a vector of cycles (each cycle is a vector of agent names).
    ///
    /// Example: vec![vec!["A", "B", "C"], vec!["X", "Y"]]
    pub fn detect_potential_deadlocks(&self) -> Vec<Vec<String>> {
        // Build a dependency graph where an edge A→B means B depends on A
        let mut dependency_graph = petgraph::Graph::<String, ()>::new();
        let mut node_map = HashMap::new();

        // Create nodes
        for name in self.name_to_node.keys() {
            let idx = dependency_graph.add_node(name.clone());
            node_map.insert(name.clone(), idx);
        }

        // Add dependencies
        for node_idx in self.workflow.node_indices() {
            if let Some(node) = self.workflow.node_weight(node_idx) {
                let target_dep_idx = *node_map.get(&node.name).unwrap();

                // Add an edge for each incoming connection
                for source in self
                    .workflow
                    .neighbors_directed(node_idx, Direction::Incoming)
                {
                    if let Some(source_node) = self.workflow.node_weight(source) {
                        let source_dep_idx = *node_map.get(&source_node.name).unwrap();
                        dependency_graph.add_edge(source_dep_idx, target_dep_idx, ());
                    }
                }
            }
        }

        // Find strongly connected components (cycles in the dependency graph)
        let sccs = petgraph::algo::kosaraju_scc(&dependency_graph);

        // Return only the non-trivial SCCs (size > 1)
        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                scc.into_iter()
                    .map(|idx| dependency_graph[idx].clone())
                    .collect()
            })
            .collect()
    }
}

/// Edge weight to represent the flow of data between agents
#[allow(clippy::type_complexity)]
#[derive(Clone, Default)]
pub struct Flow {
    /// Optional transformation function to apply to the output before passing to the next agent
    pub transform: Option<Arc<dyn Fn(String) -> String + Send + Sync>>,
    /// Optional condition to determine if this flow should be taken
    pub condition: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
}

/// Node weight for the graph
#[derive(Debug)]
pub struct AgentNode {
    pub name: String,
    /// Cache for execution results
    pub last_result: Mutex<Option<Result<String, GraphWorkflowError>>>,
}

#[derive(Clone, Debug, Error)]
pub enum GraphWorkflowError {
    #[error("Agent Error: {0}")]
    AgentError(String),
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Cycle detected in workflow")]
    CycleDetected,
    #[error("Timeout executing agent: {0}")]
    Timeout(String),
    #[error("Deadlock detected in workflow execution")]
    Deadlock,
    #[error("Workflow execution canceled")]
    Canceled,
}

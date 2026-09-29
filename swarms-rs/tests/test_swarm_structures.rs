use chrono::Local;
use swarms_rs::structs::swarm::{AgentOutputSchema, MetadataSchema, MetadataSchemaMap};
use uuid::Uuid;

#[test]
fn test_metadata_schema_map_creation() {
    let _map = MetadataSchemaMap::default();
    // Map created successfully
}

#[test]
fn test_metadata_schema_map_add() {
    let map = MetadataSchemaMap::default();

    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: "Test description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    map.add("task1", schema);
    // Add completed successfully
}

#[test]
fn test_metadata_schema_map_multiple_tasks() {
    let map = MetadataSchemaMap::default();

    for i in 0..5 {
        let schema = MetadataSchema {
            swarm_id: Uuid::new_v4(),
            task: format!("Task {}", i),
            description: format!("Description {}", i),
            agents_output_schema: vec![],
            timestamp: Local::now(),
        };
        map.add(format!("task{}", i), schema);
    }

    // All tasks added successfully
}

#[test]
fn test_metadata_schema_default() {
    let schema = MetadataSchema::default();

    assert!(schema.task.is_empty());
    assert!(schema.description.is_empty());
    assert_eq!(schema.agents_output_schema.len(), 0);
}

#[test]
fn test_metadata_schema_with_agents() {
    let agent_output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "Agent1".to_string(),
        task: "Task1".to_string(),
        output: "Output1".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 100,
    };

    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: "Test description".to_string(),
        agents_output_schema: vec![agent_output],
        timestamp: Local::now(),
    };

    assert_eq!(schema.agents_output_schema.len(), 1);
    assert_eq!(schema.agents_output_schema[0].agent_name, "Agent1");
}

#[test]
fn test_agent_output_schema_creation() {
    let start = Local::now();
    let end = Local::now();

    let output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "TestAgent".to_string(),
        task: "Test task".to_string(),
        output: "Test output".to_string(),
        start,
        end,
        duration: 100,
    };

    assert_eq!(output.agent_name, "TestAgent");
    assert_eq!(output.task, "Test task");
    assert_eq!(output.output, "Test output");
    assert_eq!(output.duration, 100);
}

#[test]
fn test_agent_output_schema_unique_run_ids() {
    let output1 = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "Agent1".to_string(),
        task: "Task1".to_string(),
        output: "Output1".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 50,
    };

    let output2 = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "Agent2".to_string(),
        task: "Task2".to_string(),
        output: "Output2".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 75,
    };

    assert_ne!(output1.run_id, output2.run_id);
}

#[test]
fn test_metadata_schema_serialization() {
    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: "Test description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    let serialized = serde_json::to_string(&schema).unwrap();
    assert!(!serialized.is_empty());
    assert!(serialized.contains("Test task"));
    assert!(serialized.contains("Test description"));
}

#[test]
fn test_agent_output_schema_serialization() {
    let output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "TestAgent".to_string(),
        task: "Test task".to_string(),
        output: "Test output".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 100,
    };

    let serialized = serde_json::to_string(&output).unwrap();
    assert!(!serialized.is_empty());
    assert!(serialized.contains("TestAgent"));
    assert!(serialized.contains("Test task"));
    assert!(serialized.contains("Test output"));
}

#[test]
fn test_metadata_schema_map_serialization() {
    let map = MetadataSchemaMap::default();

    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: "Test description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    map.add("task1", schema);

    let serialized = serde_json::to_string(&map).unwrap();
    assert!(!serialized.is_empty());
}

#[test]
fn test_metadata_schema_clone() {
    let schema1 = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: "Test description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    let schema2 = schema1.clone();

    assert_eq!(schema1.swarm_id, schema2.swarm_id);
    assert_eq!(schema1.task, schema2.task);
    assert_eq!(schema1.description, schema2.description);
}

#[test]
fn test_agent_output_schema_clone() {
    let output1 = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "Agent1".to_string(),
        task: "Task1".to_string(),
        output: "Output1".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 50,
    };

    let output2 = output1.clone();

    assert_eq!(output1.run_id, output2.run_id);
    assert_eq!(output1.agent_name, output2.agent_name);
    assert_eq!(output1.task, output2.task);
}

#[test]
fn test_metadata_schema_with_multiple_agents() {
    let outputs = vec![
        AgentOutputSchema {
            run_id: Uuid::new_v4(),
            agent_name: "Agent1".to_string(),
            task: "Task".to_string(),
            output: "Output1".to_string(),
            start: Local::now(),
            end: Local::now(),
            duration: 50,
        },
        AgentOutputSchema {
            run_id: Uuid::new_v4(),
            agent_name: "Agent2".to_string(),
            task: "Task".to_string(),
            output: "Output2".to_string(),
            start: Local::now(),
            end: Local::now(),
            duration: 75,
        },
        AgentOutputSchema {
            run_id: Uuid::new_v4(),
            agent_name: "Agent3".to_string(),
            task: "Task".to_string(),
            output: "Output3".to_string(),
            start: Local::now(),
            end: Local::now(),
            duration: 100,
        },
    ];

    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Multi-agent task".to_string(),
        description: "Task executed by multiple agents".to_string(),
        agents_output_schema: outputs,
        timestamp: Local::now(),
    };

    assert_eq!(schema.agents_output_schema.len(), 3);
}

#[test]
fn test_metadata_schema_map_update_existing_task() {
    let map = MetadataSchemaMap::default();

    let schema1 = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Task 1".to_string(),
        description: "First description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    map.add("task1", schema1);

    // Update with new schema
    let schema2 = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Task 1 Updated".to_string(),
        description: "Updated description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    map.add("task1", schema2);
    // Update completed successfully
}

#[test]
fn test_metadata_schema_with_long_task_description() {
    let long_description = "a".repeat(10000);

    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: long_description.clone(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    assert_eq!(schema.description, long_description);
}

#[test]
fn test_agent_output_schema_with_special_characters() {
    let output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "Agent 🚀".to_string(),
        task: "Task with 你好世界".to_string(),
        output: "Output with @#$%".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 100,
    };

    assert!(output.agent_name.contains("🚀"));
    assert!(output.task.contains("你好世界"));
    assert!(output.output.contains("@#$%"));
}

#[test]
fn test_metadata_schema_map_clone() {
    let map1 = MetadataSchemaMap::default();

    let schema = MetadataSchema {
        swarm_id: Uuid::new_v4(),
        task: "Test task".to_string(),
        description: "Test description".to_string(),
        agents_output_schema: vec![],
        timestamp: Local::now(),
    };

    map1.add("task1", schema);

    let _map2 = map1.clone();
    // Clone completed successfully
}

#[test]
fn test_agent_output_schema_zero_duration() {
    let output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "FastAgent".to_string(),
        task: "Quick task".to_string(),
        output: "Instant output".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 0,
    };

    assert_eq!(output.duration, 0);
}

#[test]
fn test_agent_output_schema_long_duration() {
    let output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: "SlowAgent".to_string(),
        task: "Long task".to_string(),
        output: "Delayed output".to_string(),
        start: Local::now(),
        end: Local::now(),
        duration: 3600, // 1 hour in seconds
    };

    assert_eq!(output.duration, 3600);
}

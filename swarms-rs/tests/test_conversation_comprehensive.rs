use swarms_rs::structs::conversation::{
    AgentConversation, AgentShortMemory, Content, Message, Role, SwarmConversation,
};

use tempfile::tempdir;

#[test]
fn test_agent_conversation_creation() {
    let conversation = AgentConversation::new("TestAgent".to_string());
    assert_eq!(conversation.history.len(), 0);
}

#[test]
fn test_agent_conversation_with_max_messages() {
    let conversation = AgentConversation::with_max_messages("TestAgent".to_string(), Some(10));
    assert_eq!(conversation.history.len(), 0);
}

#[test]
fn test_agent_conversation_add_message() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());
    conversation.add(Role::User("User".to_string()), "Hello".to_string());

    assert_eq!(conversation.history.len(), 1);
    assert!(matches!(conversation.history[0].role, Role::User(_)));
}

#[test]
fn test_agent_conversation_add_multiple_messages() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Hello".to_string());
    conversation.add(
        Role::Assistant("Assistant".to_string()),
        "Hi there!".to_string(),
    );
    conversation.add(Role::User("User".to_string()), "How are you?".to_string());

    assert_eq!(conversation.history.len(), 3);
}

#[test]
fn test_agent_conversation_max_messages_limit() {
    let mut conversation = AgentConversation::with_max_messages("TestAgent".to_string(), Some(3));

    // Add 5 messages, but only 3 should be kept
    for i in 0..5 {
        conversation.add(Role::User("User".to_string()), format!("Message {}", i));
    }

    assert_eq!(conversation.history.len(), 3);
}

#[test]
fn test_agent_conversation_delete_message() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Message 1".to_string());
    conversation.add(Role::User("User".to_string()), "Message 2".to_string());
    conversation.add(Role::User("User".to_string()), "Message 3".to_string());

    assert_eq!(conversation.history.len(), 3);

    conversation.delete(1);
    assert_eq!(conversation.history.len(), 2);
}

#[test]
fn test_agent_conversation_update_message() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Original".to_string());
    assert_eq!(conversation.history.len(), 1);

    conversation.update(
        0,
        Role::User("User".to_string()),
        Content::Text("Updated".to_string()),
    );

    match &conversation.history[0].content {
        Content::Text(text) => assert!(text.contains("Updated")),
        other => panic!("expected text content, got {other:?}"),
    }
}

#[test]
fn test_agent_conversation_query_message() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Test message".to_string());

    let message = conversation.query(0);
    assert!(matches!(message.role, Role::User(_)));
}

#[test]
fn test_agent_conversation_search() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Hello world".to_string());
    conversation.add(
        Role::Assistant("Assistant".to_string()),
        "Hi there".to_string(),
    );
    conversation.add(Role::User("User".to_string()), "Test world".to_string());

    let results = conversation.search("world");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_agent_conversation_search_no_results() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Hello".to_string());

    let results = conversation.search("nonexistent");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_agent_conversation_clear() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Message 1".to_string());
    conversation.add(Role::User("User".to_string()), "Message 2".to_string());

    assert_eq!(conversation.history.len(), 2);

    conversation.clear();
    assert_eq!(conversation.history.len(), 0);
}

#[test]
fn test_agent_conversation_to_json() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Test".to_string());

    let json = conversation.to_json();
    assert!(json.is_ok());
    let json_str = json.unwrap();
    assert!(json_str.contains("Test"));
}

#[tokio::test]
async fn test_agent_conversation_export_to_file() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Test message".to_string());

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("conversation.txt");

    let result = conversation.export_to_file(&file_path).await;
    assert!(result.is_ok());
    assert!(file_path.exists());
}

#[test]
fn test_agent_conversation_count_messages_by_role() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Message 1".to_string());
    conversation.add(Role::User("User".to_string()), "Message 2".to_string());
    conversation.add(
        Role::Assistant("Assistant".to_string()),
        "Response 1".to_string(),
    );

    let counts = conversation.count_messages_by_role();

    assert_eq!(counts.get("User(User)"), Some(&2));
    assert_eq!(counts.get("Assistant(Assistant)"), Some(&1));
}

#[test]
fn test_agent_conversation_display() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Hello".to_string());
    conversation.add(Role::Assistant("Assistant".to_string()), "Hi".to_string());

    let display = format!("{}", conversation);
    assert!(display.contains("User(User)"));
    assert!(display.contains("Assistant(Assistant)"));
}

#[test]
fn test_agent_short_memory_creation() {
    let memory = AgentShortMemory::new();
    assert_eq!(memory.0.len(), 0);
}

#[test]
fn test_agent_short_memory_add() {
    let memory = AgentShortMemory::new();

    memory.add(
        "task1",
        "Agent1",
        Role::User("User".to_string()),
        "Test message",
    );

    assert_eq!(memory.0.len(), 1);
    assert!(memory.0.contains_key("task1"));
}

#[test]
fn test_agent_short_memory_multiple_tasks() {
    let memory = AgentShortMemory::new();

    memory.add(
        "task1",
        "Agent1",
        Role::User("User".to_string()),
        "Message 1",
    );
    memory.add(
        "task2",
        "Agent2",
        Role::User("User".to_string()),
        "Message 2",
    );

    assert_eq!(memory.0.len(), 2);
}

#[test]
fn test_agent_short_memory_same_task_multiple_messages() {
    let memory = AgentShortMemory::new();

    memory.add(
        "task1",
        "Agent1",
        Role::User("User".to_string()),
        "Message 1",
    );
    memory.add(
        "task1",
        "Agent1",
        Role::Assistant("Assistant".to_string()),
        "Response 1",
    );

    assert_eq!(memory.0.len(), 1);

    let conversation = memory.0.get("task1").unwrap();
    assert_eq!(conversation.history.len(), 2);
}

#[test]
fn test_role_display_user() {
    let role = Role::User("TestUser".to_string());
    assert_eq!(format!("{}", role), "TestUser(User)");
}

#[test]
fn test_role_display_assistant() {
    let role = Role::Assistant("TestAssistant".to_string());
    assert_eq!(format!("{}", role), "TestAssistant(Assistant)");
}

#[test]
fn test_content_display() {
    let content = Content::Text("Test content".to_string());
    assert_eq!(format!("{}", content), "Test content");
}

#[test]
fn test_message_creation() {
    let message = Message {
        role: Role::User("User".to_string()),
        content: Content::Text("Test".to_string()),
    };

    assert!(matches!(message.role, Role::User(_)));
    assert!(matches!(message.content, Content::Text(_)));
}

#[test]
fn test_swarm_conversation_creation() {
    let conversation = SwarmConversation::new();
    assert_eq!(conversation.logs.len(), 0);
}

#[test]
fn test_swarm_conversation_add_log() {
    let mut conversation = SwarmConversation::new();

    conversation.add_log(
        "Agent1".to_string(),
        "Task1".to_string(),
        "Response1".to_string(),
    );

    assert_eq!(conversation.logs.len(), 1);
}

#[test]
fn test_swarm_conversation_multiple_logs() {
    let mut conversation = SwarmConversation::new();

    conversation.add_log(
        "Agent1".to_string(),
        "Task1".to_string(),
        "Response1".to_string(),
    );
    conversation.add_log(
        "Agent2".to_string(),
        "Task2".to_string(),
        "Response2".to_string(),
    );

    assert_eq!(conversation.logs.len(), 2);
}

#[test]
fn test_swarm_conversation_default() {
    let conversation = SwarmConversation::default();
    assert_eq!(conversation.logs.len(), 0);
}

#[test]
fn test_role_equality() {
    let role1 = Role::User("User".to_string());
    let role2 = Role::User("User".to_string());
    let role3 = Role::Assistant("Assistant".to_string());

    assert_eq!(role1, role2);
    assert_ne!(role1, role3);
}

#[test]
fn test_content_equality() {
    let content1 = Content::Text("Test".to_string());
    let content2 = Content::Text("Test".to_string());
    let content3 = Content::Text("Different".to_string());

    assert_eq!(content1, content2);
    assert_ne!(content1, content3);
}

#[test]
fn test_agent_conversation_serialization() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Test".to_string());

    // Test serialization
    let serialized = serde_json::to_string(&conversation).unwrap();
    assert!(!serialized.is_empty());
    assert!(serialized.contains("TestAgent"));
}

#[test]
fn test_agent_short_memory_default() {
    let memory = AgentShortMemory::default();
    assert_eq!(memory.0.len(), 0);
}

#[test]
fn test_agent_conversation_message_timestamps() {
    let mut conversation = AgentConversation::new("TestAgent".to_string());

    conversation.add(Role::User("User".to_string()), "Message 1".to_string());

    // Check that the message contains a timestamp
    match &conversation.history[0].content {
        Content::Text(text) => {
            assert!(text.contains("Timestamp(millis):"));
        },
        other => panic!("expected text content, got {other:?}"),
    }
}

#[test]
fn test_agent_conversation_unlimited_messages() {
    let mut conversation = AgentConversation::with_max_messages("TestAgent".to_string(), None);

    // Add many messages
    for i in 0..1000 {
        conversation.add(Role::User("User".to_string()), format!("Message {}", i));
    }

    // All messages should be kept
    assert_eq!(conversation.history.len(), 1000);
}

#[test]
fn test_role_clone() {
    let role1 = Role::User("User".to_string());
    let role2 = role1.clone();

    assert_eq!(role1, role2);
}

#[test]
fn test_content_clone() {
    let content1 = Content::Text("Test".to_string());
    let content2 = content1.clone();

    assert_eq!(content1, content2);
}

#[test]
fn test_message_clone() {
    let message1 = Message {
        role: Role::User("User".to_string()),
        content: Content::Text("Test".to_string()),
    };
    let message2 = message1.clone();

    assert_eq!(message1.role, message2.role);
    assert_eq!(message1.content, message2.content);
}

#[test]
fn test_agent_conversation_empty_search() {
    let conversation = AgentConversation::new("TestAgent".to_string());
    let results = conversation.search("anything");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_swarm_conversation_serialization() {
    let mut conversation = SwarmConversation::new();
    conversation.add_log(
        "Agent1".to_string(),
        "Task1".to_string(),
        "Response1".to_string(),
    );

    let serialized = serde_json::to_string(&conversation).unwrap();
    assert!(!serialized.is_empty());
    assert!(serialized.contains("Agent1"));
}

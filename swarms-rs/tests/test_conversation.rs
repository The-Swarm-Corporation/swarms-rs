use std::path::Path;
use swarms_rs::structs::conversation::{
    AgentConversation, AgentLog, Content, Message, Role, SwarmConversation,
};
use tempfile::TempDir;

#[test]
fn test_agent_conversation_new() {
    let conversation = AgentConversation::new("test_agent".to_string());
    // Note: agent_name is private, so we test behavior through public methods
    assert!(conversation.history.is_empty());
    // max_messages is also private, we test its behavior through add() method
}

#[test]
fn test_agent_conversation_with_max_messages() {
    let conversation = AgentConversation::with_max_messages("test_agent".to_string(), Some(100));
    // Note: agent_name and max_messages are private, so we test behavior through public methods
    assert!(conversation.history.is_empty());

    let conversation_no_limit =
        AgentConversation::with_max_messages("test_agent".to_string(), None);
    assert!(conversation_no_limit.history.is_empty());
}

#[test]
fn test_agent_conversation_add_message() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Hello".to_string());
    assert_eq!(conversation.history.len(), 1);

    conversation.add(
        Role::Assistant("assistant1".to_string()),
        "Hi there".to_string(),
    );
    assert_eq!(conversation.history.len(), 2);

    // Check that timestamps are added
    let first_message = &conversation.history[0];
    let Content::Text(ref text) = first_message.content else {
        panic!("expected text content")
    };
    assert!(text.contains("Timestamp(millis):"));
    assert!(text.contains("Hello"));
}

#[test]
fn test_agent_conversation_max_messages_limit() {
    let mut conversation = AgentConversation::with_max_messages("test_agent".to_string(), Some(2));

    conversation.add(Role::User("user1".to_string()), "Message 1".to_string());
    conversation.add(Role::User("user1".to_string()), "Message 2".to_string());
    assert_eq!(conversation.history.len(), 2);

    // Adding third message should remove the first one
    conversation.add(Role::User("user1".to_string()), "Message 3".to_string());
    assert_eq!(conversation.history.len(), 2);

    // First message should be removed, second and third should remain
    let Content::Text(ref text) = conversation.history[0].content else {
        panic!("expected text content")
    };
    assert!(text.contains("Message 2"));

    let Content::Text(ref text) = conversation.history[1].content else {
        panic!("expected text content")
    };
    assert!(text.contains("Message 3"));
}

#[test]
fn test_agent_conversation_delete_message() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Message 1".to_string());
    conversation.add(Role::User("user1".to_string()), "Message 2".to_string());
    conversation.add(Role::User("user1".to_string()), "Message 3".to_string());
    assert_eq!(conversation.history.len(), 3);

    conversation.delete(1); // Delete middle message
    assert_eq!(conversation.history.len(), 2);

    // Check that correct messages remain
    let Content::Text(ref text) = conversation.history[0].content else {
        panic!("expected text content")
    };
    assert!(text.contains("Message 1"));

    let Content::Text(ref text) = conversation.history[1].content else {
        panic!("expected text content")
    };
    assert!(text.contains("Message 3"));
}

#[test]
fn test_agent_conversation_update_message() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(
        Role::User("user1".to_string()),
        "Original message".to_string(),
    );
    assert_eq!(conversation.history.len(), 1);

    conversation.update(
        0,
        Role::Assistant("assistant1".to_string()),
        Content::Text("Updated message".to_string()),
    );

    // Check that message was updated
    assert_eq!(
        conversation.history[0].role,
        Role::Assistant("assistant1".to_string())
    );
    let Content::Text(ref text) = conversation.history[0].content else {
        panic!("expected text content")
    };
    assert_eq!(text, "Updated message");
}

#[test]
fn test_agent_conversation_query_message() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Test message".to_string());

    let message = conversation.query(0);
    assert_eq!(message.role, Role::User("user1".to_string()));
    let Content::Text(ref text) = message.content else {
        panic!("expected text content")
    };
    assert!(text.contains("Test message"));
}

#[test]
fn test_agent_conversation_search() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Hello world".to_string());
    conversation.add(
        Role::Assistant("assistant1".to_string()),
        "Hi there".to_string(),
    );
    conversation.add(Role::User("user1".to_string()), "How are you?".to_string());
    conversation.add(
        Role::Assistant("assistant1".to_string()),
        "I'm doing well, thanks!".to_string(),
    );

    let results = conversation.search("Hello");
    assert_eq!(results.len(), 1);

    let results = conversation.search("you");
    // Note: The search looks in the full content including timestamps, so "you" might appear in different contexts
    assert!(results.len() >= 1); // At least one match should be found

    let results = conversation.search("nonexistent");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_agent_conversation_clear() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Message 1".to_string());
    conversation.add(Role::User("user1".to_string()), "Message 2".to_string());
    assert_eq!(conversation.history.len(), 2);

    conversation.clear();
    assert_eq!(conversation.history.len(), 0);
}

#[test]
fn test_agent_conversation_to_json() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Hello".to_string());
    conversation.add(Role::Assistant("assistant1".to_string()), "Hi".to_string());

    let json_result = conversation.to_json();
    assert!(json_result.is_ok());

    let json_string = json_result.unwrap();
    assert!(json_string.contains("Hello"));
    assert!(json_string.contains("Hi"));
}

#[tokio::test]
async fn test_agent_conversation_export_import() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("conversation.txt");

    let mut conversation = AgentConversation::new("test_agent".to_string());
    conversation.add(Role::User("User".to_string()), "Hello".to_string());
    conversation.add(
        Role::Assistant("Assistant".to_string()),
        "Hi there\nOn two lines".to_string(),
    );

    conversation.export_to_file(&file_path).await.unwrap();

    let mut new_conversation = AgentConversation::new("imported_agent".to_string());
    new_conversation.import_from_file(&file_path).await.unwrap();

    // Round-trips exactly: same roles, same (multi-line) content.
    assert_eq!(new_conversation.history.len(), 2);
    assert_eq!(new_conversation.to_string(), conversation.to_string());
    assert_eq!(
        new_conversation.history[1].role,
        Role::Assistant("Assistant".to_string())
    );
}

#[test]
fn test_agent_conversation_max_messages_zero_does_not_panic() {
    let mut conversation = AgentConversation::with_max_messages("a".to_string(), Some(0));
    conversation.add(Role::User("u".to_string()), "one".to_string());
    conversation.add(Role::User("u".to_string()), "two".to_string());
    assert!(conversation.history.len() <= 1);
}

#[test]
fn test_agent_conversation_count_messages_by_role() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Message 1".to_string());
    conversation.add(Role::User("user2".to_string()), "Message 2".to_string());
    conversation.add(
        Role::Assistant("assistant1".to_string()),
        "Response 1".to_string(),
    );
    conversation.add(Role::User("user1".to_string()), "Message 3".to_string());

    let counts = conversation.count_messages_by_role();
    assert_eq!(counts.get("user1(User)"), Some(&2));
    assert_eq!(counts.get("user2(User)"), Some(&1));
    assert_eq!(counts.get("assistant1(Assistant)"), Some(&1));
}

#[test]
fn test_agent_conversation_display() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Hello".to_string());
    conversation.add(Role::Assistant("assistant1".to_string()), "Hi".to_string());

    let display_string = format!("{}", conversation);
    assert!(display_string.contains("user1(User):"));
    assert!(display_string.contains("assistant1(Assistant):"));
    assert!(display_string.contains("Hello"));
    assert!(display_string.contains("Hi"));
}

#[test]
fn test_message_creation() {
    let message = Message {
        role: Role::User("test_user".to_string()),
        content: Content::Text("Test content".to_string()),
    };

    assert_eq!(message.role, Role::User("test_user".to_string()));
    let Content::Text(text) = message.content else {
        panic!("expected text content")
    };
    assert_eq!(text, "Test content");
}

#[test]
fn test_role_display() {
    let user_role = Role::User("john".to_string());
    let assistant_role = Role::Assistant("ai".to_string());

    assert_eq!(format!("{}", user_role), "john(User)");
    assert_eq!(format!("{}", assistant_role), "ai(Assistant)");
}

#[test]
fn test_content_display() {
    let content = Content::Text("Hello world".to_string());
    assert_eq!(format!("{}", content), "Hello world");
}

#[test]
fn test_role_equality() {
    let role1 = Role::User("user1".to_string());
    let role2 = Role::User("user1".to_string());
    let role3 = Role::User("user2".to_string());
    let role4 = Role::Assistant("user1".to_string());

    assert_eq!(role1, role2);
    assert_ne!(role1, role3);
    assert_ne!(role1, role4);
}

#[test]
fn test_content_equality() {
    let content1 = Content::Text("test".to_string());
    let content2 = Content::Text("test".to_string());
    let content3 = Content::Text("different".to_string());

    assert_eq!(content1, content2);
    assert_ne!(content1, content3);
}

#[test]
fn test_swarm_conversation_new() {
    let swarm_conversation = SwarmConversation::new();
    assert!(swarm_conversation.logs.is_empty());
}

#[test]
fn test_swarm_conversation_default() {
    let swarm_conversation = SwarmConversation::default();
    assert!(swarm_conversation.logs.is_empty());
}

#[test]
fn test_swarm_conversation_add_log() {
    let mut swarm_conversation = SwarmConversation::new();

    swarm_conversation.add_log(
        "agent1".to_string(),
        "task1".to_string(),
        "response1".to_string(),
    );

    assert_eq!(swarm_conversation.logs.len(), 1);

    let log = &swarm_conversation.logs[0];
    assert_eq!(log.agent_name, "agent1");
    assert_eq!(log.task, "task1");
    assert_eq!(log.response, "response1");
}

#[test]
fn test_swarm_conversation_multiple_logs() {
    let mut swarm_conversation = SwarmConversation::new();

    swarm_conversation.add_log(
        "agent1".to_string(),
        "task1".to_string(),
        "response1".to_string(),
    );
    swarm_conversation.add_log(
        "agent2".to_string(),
        "task2".to_string(),
        "response2".to_string(),
    );
    swarm_conversation.add_log(
        "agent3".to_string(),
        "task3".to_string(),
        "response3".to_string(),
    );

    assert_eq!(swarm_conversation.logs.len(), 3);

    // Check that logs are in order (VecDeque with push_back)
    assert_eq!(swarm_conversation.logs[0].agent_name, "agent1");
    assert_eq!(swarm_conversation.logs[1].agent_name, "agent2");
    assert_eq!(swarm_conversation.logs[2].agent_name, "agent3");
}

#[test]
fn test_agent_log_creation() {
    let log = AgentLog {
        agent_name: "test_agent".to_string(),
        task: "test_task".to_string(),
        response: "test_response".to_string(),
    };

    assert_eq!(log.agent_name, "test_agent");
    assert_eq!(log.task, "test_task");
    assert_eq!(log.response, "test_response");
}

#[test]
fn test_agent_conversation_to_completion_messages() {
    let mut conversation = AgentConversation::new("test_agent".to_string());

    conversation.add(Role::User("user1".to_string()), "Hello".to_string());
    conversation.add(
        Role::Assistant("assistant1".to_string()),
        "Hi there".to_string(),
    );

    let completion_messages: Vec<swarms_rs::llm::completion::Message> = (&conversation).into();
    assert_eq!(completion_messages.len(), 2);

    // The actual content will include timestamps, so we just check that conversion works
    assert!(!completion_messages.is_empty());
}

#[test]
fn test_conversation_with_no_max_limit() {
    let mut conversation = AgentConversation::with_max_messages("test_agent".to_string(), None);

    // Add many messages without limit
    for i in 0..1000 {
        conversation.add(Role::User("user".to_string()), format!("Message {}", i));
    }

    assert_eq!(conversation.history.len(), 1000);
}

#[test]
fn test_conversation_serialization() {
    let mut conversation = AgentConversation::new("test_agent".to_string());
    conversation.add(Role::User("user1".to_string()), "Hello".to_string());

    // Test that the conversation can be serialized (it implements Serialize)
    let json_result = serde_json::to_string(&conversation);
    assert!(json_result.is_ok());
}

#[test]
fn test_message_serialization() {
    let message = Message {
        role: Role::User("test_user".to_string()),
        content: Content::Text("Test message".to_string()),
    };

    // Test that messages can be serialized and deserialized
    let json = serde_json::to_string(&message).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();

    assert_eq!(message.role, deserialized.role);
    assert_eq!(message.content, deserialized.content);
}

#[test]
fn test_swarm_conversation_serialization() {
    let mut swarm_conversation = SwarmConversation::new();
    swarm_conversation.add_log(
        "agent1".to_string(),
        "task1".to_string(),
        "response1".to_string(),
    );

    // Test that swarm conversation can be serialized
    let json_result = serde_json::to_string(&swarm_conversation);
    assert!(json_result.is_ok());

    let json = json_result.unwrap();
    assert!(json.contains("agent1"));
    assert!(json.contains("task1"));
    assert!(json.contains("response1"));
}

#[tokio::test]
async fn test_conversation_error_handling() {
    // Test error handling by trying to import from non-existent file
    let mut conversation = AgentConversation::new("test_agent".to_string());
    let non_existent_path = Path::new("/non/existent/path.txt");

    let import_result = conversation.import_from_file(non_existent_path).await;
    assert!(import_result.is_err());
}

#[test]
fn test_role_debug() {
    let user_role = Role::User("test_user".to_string());
    let assistant_role = Role::Assistant("test_assistant".to_string());

    let user_debug = format!("{:?}", user_role);
    let assistant_debug = format!("{:?}", assistant_role);

    assert!(user_debug.contains("User"));
    assert!(user_debug.contains("test_user"));
    assert!(assistant_debug.contains("Assistant"));
    assert!(assistant_debug.contains("test_assistant"));
}

#[test]
fn test_content_debug() {
    let content = Content::Text("debug test".to_string());
    let debug_output = format!("{:?}", content);

    assert!(debug_output.contains("Text"));
    assert!(debug_output.contains("debug test"));
}

#[tokio::test]
async fn test_json_round_trip_keeps_header_like_lines() {
    // Agent outputs are whole transcripts, so message bodies often contain lines that
    // look like `Name(User): ...` headers; the text format can't tell them apart.
    let mut conversation = AgentConversation::new("agent".to_string());
    conversation.add(
        Role::Assistant("Writer".to_string()),
        "Summary of the run:\nBob(User): what is 2+2?\nCalc(Assistant): 4".to_string(),
    );

    let mut restored = AgentConversation::new("agent".to_string());
    restored
        .load_json(&conversation.to_json().unwrap())
        .unwrap();
    assert_eq!(restored.history.len(), 1);
    assert_eq!(restored.to_string(), conversation.to_string());

    // import_from_file accepts the same JSON.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("conversation.json");
    std::fs::write(&path, conversation.to_json().unwrap()).unwrap();
    let mut imported = AgentConversation::new("agent".to_string());
    imported.import_from_file(&path).await.unwrap();
    assert_eq!(imported.history.len(), 1);
    assert_eq!(imported.to_string(), conversation.to_string());
}

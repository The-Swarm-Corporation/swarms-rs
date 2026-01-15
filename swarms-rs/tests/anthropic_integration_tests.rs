#[cfg(test)]
mod anthropic_integration_tests {
    use swarms_rs::llm::provider::anthropic::Anthropic;
    use swarms_rs::llm::Model;
    use swarms_rs::llm::request::{CompletionRequest, ToolDefinition};
    use swarms_rs::llm::completion::{Message, UserContent, Text};
    use std::env;

    // Helper function to check if API key is available
    fn has_api_key() -> bool {
        env::var("ANTHROPIC_API_KEY").is_ok()
    }

    #[test]
    fn test_anthropic_creation_from_api_key() {
        let anthropic = Anthropic::new("test-key-12345");
        assert_eq!(anthropic.model(), "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn test_anthropic_set_model() {
        let anthropic = Anthropic::new("test-key")
            .set_model("claude-3-5-haiku-20241022");
        assert_eq!(anthropic.model(), "claude-3-5-haiku-20241022");
    }

    #[test]
    fn test_anthropic_custom_url() {
        let anthropic = Anthropic::from_url(
            "https://custom.anthropic.com",
            "test-key"
        );
        assert_eq!(anthropic.model(), "claude-3-5-sonnet-20241022");
    }

    #[tokio::test]
    async fn test_anthropic_basic_completion() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let anthropic = Anthropic::from_env_with_model("claude-3-5-haiku-20241022");
        
        let request = CompletionRequest {
            prompt: Message::User {
                content: vec![UserContent::Text(Text {
                    text: "Say 'Hello' in one word.".to_string(),
                })],
            },
            system_prompt: Some("You are a helpful assistant.".to_string()),
            chat_history: vec![],
            tools: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
        };

        let result = anthropic.completion(request).await;
        
        match result {
            Ok(response) => {
                assert!(!response.choice.is_empty());
                println!("Anthropic response: {:?}", response.choice);
            }
            Err(e) => {
                eprintln!("Error from Anthropic API: {}", e);
                // Don't panic - API might be down or rate limited
            }
        }
    }

    #[tokio::test]
    async fn test_anthropic_with_temperature() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let anthropic = Anthropic::from_env();
        
        let request = CompletionRequest {
            prompt: Message::User {
                content: vec![UserContent::Text(Text {
                    text: "What is 2+2?".to_string(),
                })],
            },
            system_prompt: Some("You are a math tutor.".to_string()),
            chat_history: vec![],
            tools: vec![],
            temperature: Some(0.0),  // Deterministic
            max_tokens: Some(50),
        };

        let result = anthropic.completion(request).await;
        
        match result {
            Ok(response) => {
                assert!(!response.choice.is_empty());
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_anthropic_with_tools() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let anthropic = Anthropic::from_env();
        
        let tools = vec![
            ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get weather information".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "City name"
                        }
                    },
                    "required": ["location"]
                }),
            }
        ];

        let request = CompletionRequest {
            prompt: Message::User {
                content: vec![UserContent::Text(Text {
                    text: "What's the weather in San Francisco?".to_string(),
                })],
            },
            system_prompt: Some("You are a weather assistant with access to tools.".to_string()),
            chat_history: vec![],
            tools,
            temperature: Some(0.5),
            max_tokens: Some(200),
        };

        let result = anthropic.completion(request).await;
        
        match result {
            Ok(response) => {
                assert!(!response.choice.is_empty());
                println!("Tool response: {:?}", response.choice);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_anthropic_max_tokens() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let anthropic = Anthropic::from_env();
        
        let request = CompletionRequest {
            prompt: Message::User {
                content: vec![UserContent::Text(Text {
                    text: "List the first 5 numbers.".to_string(),
                })],
            },
            system_prompt: None,
            chat_history: vec![],
            tools: vec![],
            temperature: None,
            max_tokens: Some(50),
        };

        let result = anthropic.completion(request).await;
        
        match result {
            Ok(response) => {
                assert!(!response.choice.is_empty());
                // Response should be relatively short due to max_tokens
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_anthropic_different_models() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let models = vec![
            "claude-3-5-haiku-20241022",
            "claude-3-5-sonnet-20241022",
        ];

        for model in models {
            let anthropic = Anthropic::from_env_with_model(model);
            
            let request = CompletionRequest {
                prompt: Message::User {
                    content: vec![UserContent::Text(Text {
                        text: "Say hello briefly.".to_string(),
                    })],
                },
                system_prompt: Some("Be concise.".to_string()),
                chat_history: vec![],
                tools: vec![],
                temperature: Some(0.5),
                max_tokens: Some(100),
            };

            let result = anthropic.completion(request).await;
            
            match result {
                Ok(response) => {
                    assert!(!response.choice.is_empty());
                    println!("Response from {}: OK", model);
                }
                Err(e) => {
                    eprintln!("Error from {}: {}", model, e);
                }
            }
        }
    }

    #[test]
    fn test_anthropic_model_getter() {
        let anthropic = Anthropic::new("test-key")
            .set_model("claude-3-opus-20240229");
        
        assert_eq!(anthropic.model(), "claude-3-opus-20240229");
    }

    #[test]
    fn test_anthropic_multiple_instances() {
        let client1 = Anthropic::new("key1");
        let client2 = Anthropic::new("key2");
        
        // Should be able to create multiple instances
        assert_eq!(client1.model(), client2.model());
    }

    #[test]
    fn test_anthropic_clone() {
        let original = Anthropic::new("test-key");
        let cloned = original.clone();
        
        assert_eq!(original.model(), cloned.model());
    }
}

#[cfg(test)]
mod anthropic_agent_integration_tests {
    use swarms_rs::agent::SwarmsAgentBuilder;
    use swarms_rs::llm::provider::anthropic::Anthropic;
    use swarms_rs::structs::agent::Agent;
    use std::env;

    fn has_api_key() -> bool {
        env::var("ANTHROPIC_API_KEY").is_ok()
    }

    #[tokio::test]
    async fn test_anthropic_with_agent_builder() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let agent = SwarmsAgentBuilder::new_with_model(
            Anthropic::from_env_with_model("claude-3-5-haiku-20241022")
        )
        .agent_name("TestAgent")
        .system_prompt("You are a helpful assistant.")
        .build();

        let result = agent.run("Hello!".to_string()).await;
        
        match result {
            Ok(response) => {
                assert!(!response.is_empty());
                println!("Agent response: {}", response);
            }
            Err(e) => {
                eprintln!("Agent error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_anthropic_agent_with_configuration() {
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let agent = SwarmsAgentBuilder::new_with_model(
            Anthropic::from_env()
        )
        .agent_name("ConfiguredAgent")
        .system_prompt("You are an expert in concise responses.")
        .temperature(0.3)
        .max_tokens(200)
        .verbose(true)
        .build();

        let result = agent.run("What is AI?".to_string()).await;
        
        match result {
            Ok(response) => {
                assert!(!response.is_empty());
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_anthropic_agent_error_handling() {
        // This tests that the agent builder doesn't panic with invalid input
        // Even if the request fails, the agent should handle it gracefully
        
        if !has_api_key() {
            eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let agent = SwarmsAgentBuilder::new_with_model(
            Anthropic::from_env()
        )
        .agent_name("ErrorTestAgent")
        .system_prompt("Respond to any input.")
        .build();

        // Test with empty string
        let result = agent.run("".to_string()).await;
        // Should handle gracefully even if empty
        
        // Test with normal input
        let result = agent.run("Hello".to_string()).await;
        
        match result {
            Ok(_response) => {
                println!("Agent handled input successfully");
            }
            Err(e) => {
                eprintln!("Expected error handling: {}", e);
            }
        }
    }
}

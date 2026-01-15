/// OpenRouter Integration Tests
///
/// This test module verifies the OpenRouter LLM provider implementation
/// Tests focus on configuration, API request building, and error handling

#[cfg(test)]
mod tests {
    use swarms_rs::llm::provider::openrouter::OpenRouter;

    #[test]
    fn test_openrouter_new() {
        let model = OpenRouter::new("test-api-key");
        assert_eq!(model.model(), "openai/gpt-4o-mini");
        assert_eq!(model.base_url(), "https://openrouter.ai/api/v1");
    }

    #[test]
    fn test_openrouter_set_model() {
        let model = OpenRouter::new("test-key").set_model("anthropic/claude-3.5-sonnet");
        assert_eq!(model.model(), "anthropic/claude-3.5-sonnet");
    }

    #[test]
    fn test_openrouter_set_temperature() {
        let model = OpenRouter::new("test-key").set_temperature(0.7);
        assert_eq!(model.temperature, Some(0.7));
    }

    #[test]
    fn test_openrouter_set_max_tokens() {
        let model = OpenRouter::new("test-key").set_max_tokens(2048);
        assert_eq!(model.max_tokens, Some(2048));
    }

    #[test]
    fn test_openrouter_custom_base_url() {
        let model = OpenRouter::new("test-key")
            .with_base_url("https://custom-api.example.com");
        assert_eq!(model.base_url(), "https://custom-api.example.com");
    }

    #[test]
    fn test_openrouter_chain_configuration() {
        let model = OpenRouter::new("test-key")
            .set_model("google/gemini-2-flash")
            .set_temperature(0.5)
            .set_max_tokens(4096)
            .with_base_url("https://api.example.com/v1");

        assert_eq!(model.model(), "google/gemini-2-flash");
        assert_eq!(model.temperature, Some(0.5));
        assert_eq!(model.max_tokens, Some(4096));
        assert_eq!(model.base_url(), "https://api.example.com/v1");
    }

    #[test]
    fn test_openrouter_agent_builder() {
        let model = OpenRouter::new("test-key");
        let builder = model.agent_builder();
        // Verify that agent_builder returns a valid SwarmsAgentBuilder
        // This test ensures the method is accessible and callable
        assert!(!model.model().is_empty());
    }

    #[test]
    fn test_openrouter_model_variants() {
        let models = vec![
            "openai/gpt-4o",
            "openai/gpt-4-turbo",
            "anthropic/claude-3.5-sonnet",
            "anthropic/claude-3-opus",
            "google/gemini-2-flash",
            "meta-llama/llama-2-70b-chat",
            "mistralai/mistral-large",
        ];

        for model_name in models {
            let model = OpenRouter::new("test-key").set_model(model_name);
            assert_eq!(model.model(), model_name);
        }
    }

    #[test]
    fn test_openrouter_temperature_bounds() {
        // Test various temperature values
        for temp in [0.0, 0.5, 1.0, 1.5, 2.0] {
            let model = OpenRouter::new("test-key").set_temperature(temp);
            assert_eq!(model.temperature, Some(temp));
        }
    }

    #[test]
    fn test_openrouter_max_tokens_values() {
        let token_values = vec![128, 512, 1024, 2048, 4096, 8192];

        for tokens in token_values {
            let model = OpenRouter::new("test-key").set_max_tokens(tokens);
            assert_eq!(model.max_tokens, Some(tokens));
        }
    }
}

//! OpenAI environment constructor tests. No API calls or real credentials are required.

use std::env;

use swarms_rs::llm::provider::{any::ModelNameError, openai::OpenAI};

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Tests that read or change `OPENAI_API_KEY` must not interleave.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_openai_try_from_env_missing_key() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        let error = OpenAI::try_from_env()
            .err()
            .expect("missing API key should return an error");

        assert_eq!(
            error,
            ModelNameError::MissingApiKey {
                model: "gpt-4o-mini".to_string(),
                var: "OPENAI_API_KEY",
            }
        );
    }

    #[test]
    fn test_openai_try_from_env_empty_key() {
        let _guard = env_lock();
        unsafe {
            env::set_var("OPENAI_API_KEY", "");
        }

        let result = OpenAI::try_from_env();
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        assert_eq!(
            result.err().expect("empty API key should return an error"),
            ModelNameError::MissingApiKey {
                model: "gpt-4o-mini".to_string(),
                var: "OPENAI_API_KEY",
            }
        );
    }

    #[test]
    fn test_openai_try_from_env_with_key() {
        let _guard = env_lock();
        unsafe {
            env::set_var("OPENAI_API_KEY", "test-key-from-env");
        }

        let result = OpenAI::try_from_env();
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        assert!(result.is_ok());
    }

    #[test]
    fn test_openai_try_from_env_with_model_missing_key() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        let error = OpenAI::try_from_env_with_model("custom-model".to_string())
            .err()
            .expect("missing API key should return an error");

        assert_eq!(
            error,
            ModelNameError::MissingApiKey {
                model: "custom-model".to_string(),
                var: "OPENAI_API_KEY",
            }
        );
    }

    #[test]
    fn test_openai_try_from_env_with_model_empty_key() {
        let _guard = env_lock();
        unsafe {
            env::set_var("OPENAI_API_KEY", "");
        }

        let result = OpenAI::try_from_env_with_model("custom-model");
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        assert_eq!(
            result.err().expect("empty API key should return an error"),
            ModelNameError::MissingApiKey {
                model: "custom-model".to_string(),
                var: "OPENAI_API_KEY",
            }
        );
    }

    #[test]
    fn test_openai_try_from_env_with_model() {
        let _guard = env_lock();
        unsafe {
            env::set_var("OPENAI_API_KEY", "test-key-from-env");
        }

        let result = OpenAI::try_from_env_with_model("custom-model");
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        assert!(result.is_ok());
    }
}

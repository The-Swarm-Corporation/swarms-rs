use std::env;
use std::sync::Once;
use swarms_rs::logging::init_logger;
use swarms_rs::{log_agent, log_llm, log_memory, log_task, log_tool};

static LOGGER_INIT: Once = Once::new();

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure_logger_initialized() {
        LOGGER_INIT.call_once(|| {
            // Initialize logger with default level for tests
            unsafe {
                env::set_var("SWARMS_LOG_LEVEL", "TRACE");
            }
            init_logger();
        });
    }

    #[test]
    pub fn test_logger_initialization() {
        ensure_logger_initialized();

        // Test basic logging functionality
        log::info!("Test log message");
        log::debug!("Test debug message");
        log::error!("Test error message");

        // Verify that logging doesn't panic
    }

    #[test]
    pub fn test_agent_logging_macros() {
        ensure_logger_initialized();

        // Test all agent logging macros
        log_agent!(info, "TestAgent", "agent-123", "Agent started successfully");

        log_task!(
            info,
            "TestAgent",
            "agent-123",
            "Process data",
            "Task initialized"
        );

        log_tool!(
            debug,
            "TestAgent",
            "agent-123",
            "WebScraper",
            "Tool execution started"
        );

        log_memory!(
            trace,
            "TestAgent",
            "agent-123",
            "Save",
            "Saving to long-term memory"
        );

        log_llm!(
            info,
            "TestAgent",
            "agent-123",
            "GPT-4",
            "Sending completion request"
        );

        // Verify that all macros executed without panicking
    }

    #[test]
    pub fn test_logger_with_different_levels() {
        ensure_logger_initialized();

        // Test logging at different levels (logger is already initialized at TRACE level)
        log::trace!("Trace message");
        log::debug!("Debug message");
        log::info!("Info message");
        log::warn!("Warning message");
        log::error!("Error message");
    }
}

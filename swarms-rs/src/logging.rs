use env_logger::{Builder, Target, WriteStyle};
use log::{Level, LevelFilter};
use std::env;
use std::io::Write;

/// Initialize the logging system with environment variable support
/// Reads SWARMS_LOG_LEVEL environment variable and sets up colorful logging
pub fn init_logger() {
    let log_level = env::var("SWARMS_LOG_LEVEL")
        .unwrap_or_else(|_| "INFO".to_string())
        .to_uppercase();

    let level_filter = match log_level.as_str() {
        "TRACE" => LevelFilter::Trace,
        "DEBUG" => LevelFilter::Debug,
        "INFO" => LevelFilter::Info,
        "WARN" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,
        "OFF" => LevelFilter::Off,
        _ => {
            eprintln!(
                "⚠️  Invalid SWARMS_LOG_LEVEL '{}', defaulting to INFO",
                log_level
            );
            LevelFilter::Info
        },
    };

    let mut builder = Builder::from_default_env();
    builder
        .target(Target::Stdout)
        .write_style(WriteStyle::Always)
        .filter_level(level_filter)
        .format(|buf, record| {
            let level_str = match record.level() {
                Level::Error => "ERROR",
                Level::Warn => "WARN",
                Level::Info => "INFO",
                Level::Debug => "DEBUG",
                Level::Trace => "TRACE",
            };

            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC");
            let target = record.target();

            writeln!(
                buf,
                "{} [{}] {} - {}",
                timestamp,
                target,
                level_str,
                record.args()
            )
        })
        // try_init: calling this twice, or after another logger is installed, must not panic.
        .try_init()
        .ok();

    log::info!("🚀 Swarms-RS logging initialized with level: {}", log_level);
}

/// Macro for logging with agent context
#[macro_export]
macro_rules! log_agent {
    ($level:ident, $agent_name:expr, $agent_id:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[{}:{}] {}",
            $agent_name,
            $agent_id,
            format!($($arg)*)
        );
    };
}

/// Macro for logging task-related events with agent context
#[macro_export]
macro_rules! log_task {
    ($level:ident, $agent_name:expr, $agent_id:expr, $task:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[{}:{}] 📋 Task: {} - {}",
            $agent_name,
            $agent_id,
            $task,
            format!($($arg)*)
        );
    };
}

/// Macro for logging tool execution with agent context
#[macro_export]
macro_rules! log_tool {
    ($level:ident, $agent_name:expr, $agent_id:expr, $tool_name:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[{}:{}] 🔧 Tool: {} - {}",
            $agent_name,
            $agent_id,
            $tool_name,
            format!($($arg)*)
        );
    };
}

/// Macro for logging workflow events with agent context
#[macro_export]
macro_rules! log_workflow {
    ($level:ident, $agent_name:expr, $agent_id:expr, $workflow_name:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[{}:{}] 🔄 Workflow: {} - {}",
            $agent_name,
            $agent_id,
            $workflow_name,
            format!($($arg)*)
        );
    };
}

/// Macro for logging memory operations with agent context
#[macro_export]
macro_rules! log_memory {
    ($level:ident, $agent_name:expr, $agent_id:expr, $operation:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[{}:{}] 🧠 Memory: {} - {}",
            $agent_name,
            $agent_id,
            $operation,
            format!($($arg)*)
        );
    };
}

/// Macro for logging LLM interactions with agent context
#[macro_export]
macro_rules! log_llm {
    ($level:ident, $agent_name:expr, $agent_id:expr, $model:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[{}:{}] 🤖 LLM: {} - {}",
            $agent_name,
            $agent_id,
            $model,
            format!($($arg)*)
        );
    };
}

/// Macro for logging swarm operations
#[macro_export]
macro_rules! log_swarm {
    ($level:ident, $swarm_name:expr, $($arg:tt)*) => {
        $crate::log::$level!(
            "[🐝 Swarm: {}] {}",
            $swarm_name,
            format!($($arg)*)
        );
    };
}

/// Macro for logging performance metrics
#[macro_export]
macro_rules! log_perf {
    ($level:ident, $component:expr, $metric:expr, $value:expr, $unit:expr) => {
        $crate::log::$level!(
            "📊 Performance: {} - {}: {} {}",
            $component,
            $metric,
            $value,
            $unit
        );
    };
}

/// Macro for logging errors with context
#[macro_export]
macro_rules! log_error_ctx {
    ($agent_name:expr, $agent_id:expr, $error:expr, $context:expr) => {
        $crate::log::error!(
            "[{}:{}] ❌ Error in {}: {}",
            $agent_name,
            $agent_id,
            $context,
            $error
        );
    };
}

/// Utility function to log agent initialization
pub fn log_agent_init(agent_name: &str, agent_id: &str, config_summary: &str) {
    log::info!(
        "🎯 Agent [{}:{}] initialized with config: {}",
        agent_name,
        agent_id,
        config_summary
    );
}

/// Utility function to log agent task completion
pub fn log_agent_task_completion(agent_name: &str, agent_id: &str, task: &str, duration_ms: u64) {
    log::info!(
        "✅ Agent [{}:{}] completed task '{}' in {}ms",
        agent_name,
        agent_id,
        task,
        duration_ms
    );
}

/// Utility function to log agent state changes
pub fn log_agent_state_change(agent_name: &str, agent_id: &str, from_state: &str, to_state: &str) {
    log::debug!(
        "🔄 Agent [{}:{}] state transition: {} → {}",
        agent_name,
        agent_id,
        from_state,
        to_state
    );
}

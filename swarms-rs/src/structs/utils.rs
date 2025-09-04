use chrono::Local;
use uuid::Uuid;

use crate::structs::{
    agent::{Agent, AgentError},
    swarm::AgentOutputSchema,
};

use colored::*;

/// Global markdown rendering utility for beautiful output across all architectures
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Renders markdown text with proper formatting and colors
    pub fn render(text: &str) {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        
        while i < lines.len() {
            let line = lines[i].trim();
            
            if line.starts_with("### ") {
                // H3 headers
                let header = &line[4..];
                println!("{}", format!(" {}", header).bold().cyan());
            } else if line.starts_with("#### ") {
                // H4 headers
                let header = &line[5..];
                println!("{}", format!("   {}", header).bold().magenta());
            } else if line.starts_with("## ") {
                // H2 headers
                let header = &line[3..];
                println!("{}", format!(" {}", header).bold().blue());
            } else if line.starts_with("# ") {
                // H1 headers
                let header = &line[2..];
                println!("{}", format!(" {}", header).bold().green());
            } else if line.starts_with("**") && line.ends_with("**") {
                // Bold text
                let bold_text = &line[2..line.len()-2];
                println!("{}", bold_text.bold());
            } else if line.starts_with("- ") {
                // Bullet points
                let bullet = &line[2..];
                println!("{}", format!("  • {}", bullet).white());
            } else if line.starts_with("---") {
                // Horizontal rules
                println!("{}", "─".repeat(60).dimmed());
            } else if line.is_empty() {
                // Empty lines
                println!();
            } else if line.contains("**Strengths:**") {
                println!("{}", "   Strengths:".bold().green());
            } else if line.contains("**Weaknesses:**") {
                println!("{}", "  ⚠  Weaknesses:".bold().red());
            } else if line.contains("**Suggestions for Improvement:**") {
                println!("{}", "  & Suggestions for Improvement:".bold().yellow());
            } else if line.contains("**Overall Recommendations") {
                println!("{}", " Overall Recommendations:".bold().blue());
            } else if line.contains("**Analysis:**") {
                println!("{}", " Analysis:".bold().cyan());
            } else if line.contains("**Recommendations:**") {
                println!("{}", "& Recommendations:".bold().yellow());
            } else if line.contains("**Risk Assessment:**") {
                println!("{}", "⚠  Risk Assessment:".bold().red());
            } else if line.contains("**Market Analysis:**") {
                println!("{}", " Market Analysis:".bold().green());
            } else if line.contains("**Investment Strategy:**") {
                println!("{}", "$ Investment Strategy:".bold().blue());
            } else if line.contains("**Conclusion:**") {
                println!("{}", " Conclusion:".bold().magenta());
            } else if line.contains("**Summary:**") {
                println!("{}", " Summary:".bold().cyan());
            } else if line.contains("**Key Points:**") {
                println!("{}", " Key Points:".bold().yellow());
            } else if line.contains("**Next Steps:**") {
                println!("{}", "➡  Next Steps:".bold().green());
            } else if line.contains("**Status:**") {
                println!("{}", " Status:".bold().blue());
            } else if line.contains("**Error:**") {
                println!("{}", "X Error:".bold().red());
            } else if line.contains("**Success:**") {
                println!("{}", "v Success:".bold().green());
            } else if line.contains("**Warning:**") {
                println!("{}", "⚠  Warning:".bold().yellow());
            } else if line.contains("**Info:**") {
                println!("{}", "[INFO] Info:".bold().cyan());
            } else {
                // Regular text
                println!("{}", line.white());
            }
            
            i += 1;
        }
    }

    /// Renders a section header with consistent styling
    pub fn render_section_header(title: &str) {
        println!("{}", format!(" {}", title).bold().cyan());
        println!("{}", "─".repeat(60).dimmed());
    }

    /// Renders a subsection header
    pub fn render_subsection_header(title: &str) {
        println!("{}", format!("   {}", title).bold().magenta());
    }

    /// Renders a success message
    pub fn render_success(message: &str) {
        println!("{}", format!(" {}", message).green());
    }

    /// Renders an error message
    pub fn render_error(message: &str) {
        println!("{}", format!(" {}", message).red());
    }

    /// Renders a warning message
    pub fn render_warning(message: &str) {
        println!("{}", format!("⚠  {}", message).yellow());
    }

    /// Renders an info message
    pub fn render_info(message: &str) {
        println!("{}", format!("[INFO] {}", message).cyan());
    }

    /// Renders a progress indicator
    pub fn render_progress(current: usize, total: usize, message: &str) {
        let percentage = (current as f32 / total as f32 * 100.0) as usize;
        let progress_bar = "█".repeat(percentage / 5) + &"░".repeat(20 - percentage / 5);
        println!("{} {}% - {}", progress_bar, percentage, message);
    }

    /// Renders a workflow step
    pub fn render_workflow_step(step: usize, total: usize, step_name: &str) {
        println!("{}", format!(" Step {}/{}: {}", step, total, step_name).bold().yellow());
    }

    /// Renders agent output with enhanced borders
    /// 
    /// # Arguments
    /// * `formatter` - The formatter instance to use for rendering
    /// * `agent_name` - Name of the agent
    /// * `output` - Output content to render
    pub fn render_agent_output(formatter: &mut crate::utils::formatter::Formatter, agent_name: &str, output: &str) {
        formatter.render_agent_output(agent_name, output);
    }

    /// Renders workflow completion
    pub fn render_workflow_completion(workflow_name: &str) {
        println!("{}", format!(" {} completed successfully!", workflow_name).bold().green());
    }
}

pub async fn run_agent_with_output_schema(
    agent: &dyn Agent,
    task: String,
) -> Result<AgentOutputSchema, AgentError> {
    let start = Local::now();
    let output = agent.run(task.clone()).await?;

    let end = Local::now();
    let duration = end.signed_duration_since(start).num_seconds();

    let agent_output = AgentOutputSchema {
        run_id: Uuid::new_v4(),
        agent_name: agent.name(),
        task,
        output,
        start,
        end,
        duration,
    };

    Ok(agent_output)
}

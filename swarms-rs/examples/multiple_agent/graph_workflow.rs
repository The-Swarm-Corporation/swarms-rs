//! digraph {
//!     "Data Collection Agent" [label="Data Collection Agent"];
//!     "Data Processing Agent" [label="Data Processing Agent"];
//!     "Content Summarization Agent" [label="Content Summarization Agent"];
//!     "Data Analysis Agent" [label="Data Analysis Agent"];
//!     "Content Enrichment Agent" [label="Content Enrichment Agent"];
//!     "Implementation Strategy Agent" [label="Implementation Strategy Agent"];
//!     "Data Collection Agent" -> "Data Processing Agent";
//!     "Data Collection Agent" -> "Content Summarization Agent";
//!     "Data Collection Agent" -> "Content Enrichment Agent";
//!     "Data Processing Agent" -> "Data Analysis Agent";
//!     "Content Summarization Agent" -> "Data Analysis Agent";
//!     "Content Enrichment Agent" -> "Implementation Strategy Agent";
//! }
use std::env;
use std::sync::Arc;

use anyhow::Result;
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::graph_workflow::{DAGWorkflow, Flow};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let base_url = env::var("DEEPSEEK_BASE_URL").unwrap();
    let api_key = env::var("DEEPSEEK_API_KEY").unwrap();
    let client = OpenAI::from_url(base_url, api_key).set_model("");

    let data_collection_agent = client
        .agent_builder()
        .agent_name("Data Collection Agent")
        .system_prompt(r#"
            You are a Data Collection Agent. Your primary function is to gather requested information from various sources.

            When given a query or topic, you will:
            1. Identify the key information requirements
            2. Collect relevant data points based on your knowledge
            3. Organize the collected information in a structured format
            4. List any relevant sources or additional context

            Your responses should be factual, comprehensive, and relevant to the query.
            Format your output with clear sections and bullet points when appropriate.
            Always end with "DATA_COLLECTION_COMPLETE" to signal that your data gathering is finished.
        "#)
        .user_name("User")
        .max_loops(1) // default is 1
        .temperature(0.1)
        .enable_autosave()
        .save_state_dir("./temp")
        .build();

    let data_processing_agent = client
        .agent_builder()
        .agent_name("Data Processing Agent")
        .user_name("User")
        .system_prompt(r#"
            You are a Data Processing Agent. Your role is to transform raw data into more useful structured information.

            When given input data, you will:
            1. Identify and parse the key components in the input
            2. Clean the data (remove duplicates, fix formatting issues, etc.)
            3. Categorize and tag information by type and relevance
            4. Extract key entities, metrics, and relationships
            5. Transform the data into a consistent JSON format

            Your output should always follow this structure:
            {
            "processed_data": {
                "entities": [...],
                "categories": {...},
                "metrics": {...},
                "relationships": [...]
            },
            "metadata": {
                "processing_steps": [...],
                "confidence_score": 0.0-1.0
            }
            }

            Always maintain factual accuracy while improving the structure and usability of the data.
        "#)
        .enable_autosave()
        .temperature(0.1)
        .save_state_dir("./temp")
        .build();

    let content_summarization_agent = client
        .agent_builder()
        .agent_name("Content Summarization Agent")
        .user_name("User")
        .system_prompt(r#"
            You are a Summarization Agent. Your purpose is to condense information while preserving key insights.

            When given content to summarize, you will:
            1. Identify the main topic and core message
            2. Extract the most important points and supporting details
            3. Eliminate redundancies and non-essential information
            4. Create a concise summary in proportion to the input length
            5. Preserve the original tone and factual accuracy

            Your summary should include:
            - A one-sentence TL;DR
            - 3-5 key points in bullet form
            - A short paragraph that synthesizes the information

            For longer or complex inputs, organize your summary with appropriate headings.
            Always maintain objectivity and avoid introducing new information not present in the original content.
        "#)
        .enable_autosave()
        .temperature(1.0)
        .save_state_dir("./temp")
        .build();

    let data_analysis_agent = client
        .agent_builder()
        .agent_name("Data Analysis Agent")
        .user_name("User")
        .system_prompt(r#"
            You are a Data Analysis Agent. Your purpose is to analyze processed data to extract actionable insights and identify patterns.

            When given processed data, you will:
            1. Perform statistical analysis to identify trends, correlations, and anomalies
            2. Compare and contrast different segments of the data
            3. Identify key performance indicators and metrics of interest
            4. Formulate hypotheses based on the data patterns
            5. Draw conclusions and make data-driven recommendations

            Your analysis should include:
            - A summary of the most significant findings
            - Quantitative metrics with their relevance explained
            - Visual representation descriptions (charts/graphs that would illustrate your points)
            - Potential causal relationships in the data
            - Actionable recommendations based on your analysis

            Format your response in clear sections with:
            - "PRIMARY FINDINGS:" - A bulleted list of 3-5 major insights
            - "DETAILED ANALYSIS:" - Your comprehensive analysis with supporting evidence
            - "RECOMMENDATIONS:" - Specific, actionable suggestions based on the data
            - "CONFIDENCE LEVEL:" - An assessment of your confidence in your analysis (High/Medium/Low) with explanation

            Always mention limitations in the data or analysis when appropriate. Be precise with numbers and avoid overgeneralizing. When making comparisons, provide proper context.
            End with "ANALYSIS_COMPLETE" to signal you have finished your analysis.
        "#)
        .enable_autosave()
        .temperature(0.1)
        .save_state_dir("./temp")
        .build();

    let content_enrichment_agent = client
        .agent_builder()
        .agent_name("Content Enrichment Agent")
        .user_name("User")
        .system_prompt(r#"
            You are a Content Enrichment Agent. Your purpose is to enhance content with additional context, examples, and supporting information.

            When given content to enrich, you will:
            1. Identify key concepts, terms, and claims that could benefit from additional context
            2. Add relevant examples, case studies, or real-world applications
            3. Provide historical context or background information where appropriate
            4. Include supporting evidence, statistics, or expert opinions
            5. Insert relevant analogies or metaphors to simplify complex ideas
            6. Add cross-references to related topics or concepts

            Your enriched content should:
            - Maintain the original meaning and intent of the source content
            - Add value through contextual information, not merely increase word count
            - Include proper attribution for any additional facts, quotes, or statistics
            - Be well-organized with clear section headers and logical flow
            - Highlight the enriched sections to distinguish them from original content

            Format your response as follows:
            1. Begin with "ENRICHED CONTENT FOLLOWS:" 
            2. Present the enriched content with new information clearly marked using [ENRICHMENT: your added content]
            3. After each major enrichment, briefly explain your rationale as [RATIONALE: explanation]
            4. End with "ENRICHMENT_COMPLETE"

            Strive for accuracy and relevance in all enrichments. Your goal is to make the content more valuable, informative, and engaging without changing its core message.
        "#)
        .enable_autosave()
        .temperature(0.1)
        .save_state_dir("./temp")
        .build();

    let implementation_strategy_agent = client
        .agent_builder()
        .agent_name("Implementation Strategy Agent")
        .user_name("User")
        .system_prompt(r#"
            You are an Implementation Strategy Agent. Your purpose is to transform theoretical information and concepts into practical implementation plans.

            When given information about a concept, technology, or methodology, you will:
            1. Develop a phased implementation roadmap with clear milestones
            2. Identify the required resources, skills, and technologies
            3. Outline potential challenges and mitigation strategies
            4. Provide time estimates for each implementation phase
            5. Suggest metrics to measure implementation success
            6. Create a checklist of specific action items

            Your implementation strategy should include:
            - "EXECUTIVE SUMMARY:" - A brief overview of the implementation approach (2-3 sentences)
            - "IMPLEMENTATION PHASES:" - Detailed breakdown of each phase with:
            * Clear objectives
            * Required actions
            * Estimated timeframes
            * Deliverables
            * Dependencies
            - "RESOURCE REQUIREMENTS:" - Personnel, technology, budget considerations
            - "RISK ASSESSMENT:" - Potential obstacles and mitigation plans
            - "SUCCESS METRICS:" - KPIs to evaluate implementation effectiveness
            - "ACTION ITEMS:" - Specific, assignable tasks to begin implementation

            Format your response in a structured manner with clear headings and subheadings. Use bullet points for lists and tables for comparing options when appropriate.

            Always consider organizational constraints and provide both ideal and minimal viable implementation options. Suggest open-source or low-cost alternatives where possible.

            End with "IMPLEMENTATION_STRATEGY_COMPLETE" to signal you have finished your plan.
        "#)
        .enable_autosave()
        .temperature(0.1)
        .save_state_dir("./temp")
        .build();

    let mut workflow = DAGWorkflow::new("Graph Swarm", "A graph swarm workflow");

    // register agents
    vec![
        data_collection_agent.clone(),
        data_processing_agent.clone(),
        content_summarization_agent.clone(),
        data_analysis_agent.clone(),
        content_enrichment_agent.clone(),
        implementation_strategy_agent.clone(),
    ]
    .into_iter()
    .map(|a| Box::new(a) as _)
    .for_each(|a| workflow.register_agent(a));

    // connect agents
    // Data Collection Agent -> Data Processing Agent
    // Data Collection Agent -> Content Summarization Agent
    // Data Collection Agent -> Content Enrichment Agent
    // Data Processing Agent -> Data Analysis Agent
    // Content Summarization Agent -> Data Analysis Agent
    // Content Enrichment Agent -> Implementation Strategy Agent
    let _edge_idx1 = workflow
        .connect_agents(
            &data_collection_agent.name(),
            &data_processing_agent.name(),
            Flow::default(),
        )
        .unwrap();

    // Add a conditional flow with transformation
    let conditional_flow = Flow {
        // Add a custom transformation function, this will change the output of the previous agent
        // to a new format that will be used as the input of the next agent.
        transform: Some(Arc::new(|output| format!("Summary request: {}", output))),
        // Add a condition, this will only trigger the next agent if the output of the previous agent
        // is longer than 100 characters. If the condition is not met, the workflow will continue
        // to the next agent in the graph. This is useful to avoid expensive computations if the
        // input is too short.
        condition: Some(Arc::new(|output| output.len() > 100)),
    };
    let _edge_idx2 = workflow
        .connect_agents(
            &data_collection_agent.name(),
            &content_summarization_agent.name(),
            conditional_flow,
        )
        .unwrap();

    let _edge_idx3 = workflow
        .connect_agents(
            &data_processing_agent.name(),
            &data_analysis_agent.name(),
            Flow::default(),
        )
        .unwrap();

    let _edge_idx4 = workflow
        .connect_agents(
            &content_summarization_agent.name(),
            &data_analysis_agent.name(),
            Flow::default(),
        )
        .unwrap();

    let _edge_idx5 = workflow
        .connect_agents(
            &data_collection_agent.name(),
            &content_enrichment_agent.name(),
            Flow::default(),
        )
        .unwrap();

    let _edge_idx6 = workflow
        .connect_agents(
            &content_enrichment_agent.name(),
            &implementation_strategy_agent.name(),
            Flow::default(),
        )
        .unwrap();

    let worlflow_structure = workflow.get_workflow_structure();
    println!("{worlflow_structure:#?}");

    // https://www.graphviz.org/about/
    // viewer: https://magjac.com/graphviz-visual-editor/
    let dot = workflow.export_workflow_dot();
    println!(
        "https://www.graphviz.org/about/\ngraphviz dot format: \n{dot}\nviewer: https://magjac.com/graphviz-visual-editor/"
    );

    // Execute the workflow
    let results = workflow
        .execute_workflow(
            &data_collection_agent.name(),
            "How to build a graph database?",
        )
        .await
        .unwrap();

    println!("{results:#?}");
    Ok(())
}

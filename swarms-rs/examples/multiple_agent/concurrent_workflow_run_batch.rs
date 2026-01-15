use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflow;

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

    // Create specialized medical agents with independent roles
    let symptom_analysis_agent = client
        .agent_builder()
        .agent_name("Symptom Analysis Agent")
        .system_prompt(
            "You are a medical symptom analysis specialist. Your role is to analyze the described \
            symptoms and provide a detailed assessment of what these symptoms might indicate. \
            Focus only on the symptom patterns, their severity, duration, and potential \
            physiological implications. Do not attempt to provide a definitive diagnosis or \
            treatment recommendations. End your analysis with <DONE>.",
        )
        .user_name("Healthcare Provider")
        .max_loops(1)
        .temperature(0.2) // Lower temperature for more precise medical information
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/medical")
        .add_stop_word("<DONE>")
        .build();

    let medical_reference_agent = client
        .agent_builder()
        .agent_name("Medical Reference Agent")
        .system_prompt(
            "You are a medical reference specialist. Your role is to provide evidence-based \
            information about various medical conditions that might be relevant to the described \
            case. Present information about common conditions that match the described scenario, \
            including their typical presentations, risk factors, and general management approaches \
            from medical literature. Focus only on providing factual medical information without \
            making specific recommendations for this particular case. End your response with <DONE>.",
        )
        .user_name("Healthcare Provider")
        .max_loops(1)
        .temperature(0.3)
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/medical")
        .add_stop_word("<DONE>")
        .build();

    let patient_education_agent = client
        .agent_builder()
        .agent_name("Patient Education Agent")
        .system_prompt(
            "You are a patient education specialist. Your role is to create general educational \
            materials about health conditions and medical procedures. Based on the medical scenario \
            described, develop clear, accessible information that would be helpful for a patient \
            or their family to understand the general medical context. Focus on creating standalone \
            educational content without attempting to provide specific medical advice for this case. \
            Include general self-care recommendations and when to seek medical attention. \
            End your response with <DONE>.",
        )
        .user_name("Healthcare Provider")
        .max_loops(1)
        .temperature(0.4) // Slightly higher temperature for more natural communication
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/medical")
        .add_stop_word("<DONE>")
        .build();

    // Create a concurrent workflow with all independent medical agents
    let workflow = ConcurrentWorkflow::builder()
        .name("Medical Information Processing Workflow")
        .metadata_output_dir("./temp/concurrent_workflow/medical/workflow/metadata")
        .description(
            "A workflow for processing medical questions with independent specialized agents.",
        )
        .agents(vec![
            Box::new(symptom_analysis_agent),
            Box::new(medical_reference_agent),
            Box::new(patient_education_agent),
        ])
        .build();

    // Example medical scenarios for independent analysis
    let medical_scenarios = vec![
        "Patient presents with fever, cough, and shortness of breath for 5 days. \
         History of asthma. Provide your specialized analysis of this case.",
        "Elderly patient with sudden onset confusion, headache, and neck stiffness. \
         No history of trauma. Analyze this case from your area of expertise.",
        "Child with rash, high fever, and swollen lymph nodes for 3 days. \
         Previously healthy. Offer your specialized perspective on this scenario.",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    // Run the workflow with the medical scenarios
    let results = workflow.run_batch(medical_scenarios).await?;

    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}

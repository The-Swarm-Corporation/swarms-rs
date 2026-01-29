use futures::stream::BoxStream;
use futures::StreamExt;
use futures::channel::mpsc;
use std::time::Duration;
use swarms_rs::{Agent};
use swarms_rs::structs::agent::AgentError;

struct StreamingAgent;

impl Agent for StreamingAgent {
    fn run(&self, task: String) -> futures::future::BoxFuture<'static, Result<String, AgentError>> {
        Box::pin(async move {
            let mut stream = self.run_stream(task);
            let mut last: Option<String> = None;
            while let Some(item) = stream.next().await {
                match item {
                    Ok(v) => last = Some(v),
                    Err(e) => return Err(e),
                }
            }
            match last {
                Some(v) => Ok(v),
                None => Err(AgentError::NoChoiceFound),
            }
        })
    }

    fn run_stream(&self, task: String) -> BoxStream<'static, Result<String, AgentError>> {
        let (mut tx, rx) = mpsc::unbounded::<Result<String, AgentError>>();

        tokio::spawn(async move {
            let _ = tx.send(Ok(format!("start: {}", task))).await;
            for i in 0..3u8 {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let _ = tx.send(Ok(format!("progress {}", i))).await;
            }
            let _ = tx.send(Ok("done".to_string())).await;
        });

        rx.boxed()
    }

    fn run_multiple_tasks(&mut self, _tasks: Vec<String>) -> futures::future::BoxFuture<'static, Result<Vec<String>, AgentError>> {
        Box::pin(async move { Ok(vec![]) })
    }

    fn plan(&self, _task: String) -> futures::future::BoxFuture<'static, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn query_long_term_memory(&self, _task: String) -> futures::future::BoxFuture<'static, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn save_task_state(&self, _task: String) -> futures::future::BoxFuture<'static, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn is_response_complete(&self, _response: String) -> bool {
        true
    }

    fn id(&self) -> String {
        "streaming-agent".to_string()
    }

    fn name(&self) -> String {
        "StreamingAgent".to_string()
    }

    fn description(&self) -> String {
        "Example streaming agent that yields multiple values".to_string()
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(StreamingAgent)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = StreamingAgent;

    println!("Starting streaming agent demo...");
    let mut stream = agent.run_stream("example task".to_string());
    while let Some(item) = stream.next().await {
        match item {
            Ok(v) => println!("STREAM: {}", v),
            Err(e) => println!("STREAM ERROR: {}", e),
        }
    }

    // Optionally, use run() to get final value
    match agent.run("final task".to_string()).await {
        Ok(r) => println!("RUN result: {}", r),
        Err(e) => println!("RUN error: {}", e),
    }

    Ok(())
}

/// Agent Pooling Integration Example
///
/// This example demonstrates how to integrate agent pooling into a real-world
/// batch processing scenario, showing performance improvements and best practices.

use swarms_rs::agent::pool::{AgentPool, PoolConfig};
use std::time::Instant;

/// Simulates a batch job that processes multiple tasks using agents
async fn batch_processing_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Agent Pool Batch Processing Example\n");

    // Define your agent factory
    let factory = || {
        // In real usage, create actual SwarmsAgent instances
        // For this example, we use a simple mock
        format!("agent-{}", uuid::Uuid::new_v4())
    };

    // Create pool with appropriate configuration
    let config = PoolConfig {
        max_size: 10,
        min_size: 5,
        enable_metrics: true,
        acquire_timeout_ms: 5000,
    };

    let pool = AgentPool::with_config(config, factory);

    // Simulate batch processing of tasks
    let tasks = vec![
        "Process data batch 1",
        "Process data batch 2",
        "Process data batch 3",
        "Process data batch 4",
        "Process data batch 5",
    ];

    println!("📊 Processing {} tasks with agent pool\n", tasks.len());

    let start = Instant::now();

    // Process tasks using pool
    let handles: Vec<_> = tasks
        .iter()
        .map(|task| {
            let pool = pool.clone();
            let task = task.to_string();

            tokio::spawn(async move {
                let agent = pool.acquire().await.ok()?;
                println!("✓ Agent {} processing: {}", agent, task);

                // Simulate work
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                Some(format!("Result for: {}", task))
            })
        })
        .collect();

    // Wait for all tasks to complete
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .collect();

    let elapsed = start.elapsed();

    println!("\n✅ Batch processing completed in {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    println!("📈 Processed {} tasks", results.len());

    // Print metrics
    let metrics = pool.metrics();
    println!(
        "\n📊 Pool Metrics:\n  Available: {}\n  In Use: {}\n",
        metrics.available_count, metrics.in_use_count
    );

    Ok(())
}

/// Demonstrates concurrent access patterns with agent pool
async fn concurrent_request_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Concurrent Request Handling with Agent Pool\n");

    let pool = AgentPool::new(20, || {
        format!("agent-{}", uuid::Uuid::new_v4())
    });

    // Simulate concurrent requests (like HTTP endpoints)
    let mut handles = Vec::new();

    for request_id in 0..50 {
        let pool_clone = pool.clone();

        let handle = tokio::spawn(async move {
            match pool_clone.acquire().await {
                Ok(agent) => {
                    println!("→ Request {} assigned to agent {}", request_id, agent);

                    // Simulate request processing
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                    println!("← Request {} completed", request_id);
                    Ok(request_id)
                }
                Err(e) => {
                    eprintln!("✗ Request {} failed: {}", request_id, e);
                    Err(e)
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all requests
    let results = futures::future::join_all(handles).await;
    let successful = results.iter().filter(|r| r.is_ok()).count();

    println!("\n✅ Processed {} / {} requests successfully", successful, results.len());

    Ok(())
}

/// Demonstrates performance comparison without pooling
async fn performance_comparison() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ Performance Comparison: With vs Without Pooling\n");

    // Without pooling: Create and destroy agents repeatedly
    println!("Testing WITHOUT object pooling...");
    let start = Instant::now();

    for i in 0..100 {
        let _agent = format!("agent-{}", i);
        // Simulate agent work
        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
        drop(_agent);
    }

    let without_pool = start.elapsed();
    println!("  Time: {:.2}ms\n", without_pool.as_secs_f64() * 1000.0);

    // With pooling: Reuse agents
    println!("Testing WITH object pooling...");
    let pool = AgentPool::new(10, || {
        format!("agent-{}", uuid::Uuid::new_v4())
    });

    let start = Instant::now();

    for _ in 0..100 {
        let agent = pool.acquire().await?;
        // Simulate agent work
        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
        drop(agent);
    }

    let with_pool = start.elapsed();
    println!("  Time: {:.2}ms\n", with_pool.as_secs_f64() * 1000.0);

    // Calculate improvement
    let improvement_ratio = without_pool.as_secs_f64() / with_pool.as_secs_f64();
    println!(
        "🎯 Performance Improvement: {:.1}x faster with pooling",
        improvement_ratio
    );

    Ok(())
}

/// Demonstrates proper error handling with agent pool
async fn error_handling_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️  Error Handling with Agent Pool\n");

    let config = PoolConfig {
        max_size: 2,
        min_size: 1,
        enable_metrics: true,
        acquire_timeout_ms: 500, // Short timeout for demo
    };

    let pool = AgentPool::with_config(config, || {
        format!("agent-{}", uuid::Uuid::new_v4())
    });

    // Try to exhaust pool
    println!("Attempting to acquire more agents than pool capacity...\n");

    let _agent1 = pool.acquire().await?;
    println!("✓ Acquired agent 1");

    let _agent2 = pool.acquire().await?;
    println!("✓ Acquired agent 2");

    // This should timeout
    match pool.acquire().await {
        Ok(_) => println!("✓ Acquired agent 3"),
        Err(e) => println!("✗ Failed to acquire agent 3: {}", e),
    }

    drop(_agent1);
    println!("\n✓ Released agent 1, making space in pool");

    // Now we should succeed
    match pool.acquire().await {
        Ok(_agent3) => println!("✓ Successfully acquired agent 3 after release"),
        Err(e) => println!("✗ Failed: {}", e),
    }

    Ok(())
}

/// Demonstrates batch acquisition for better performance
async fn batch_acquisition_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Batch Acquisition Example\n");

    let pool = AgentPool::new(20, || {
        format!("agent-{}", uuid::Uuid::new_v4())
    });

    // Acquire multiple agents at once
    println!("Acquiring 5 agents in batch...");
    let agents = pool.acquire_batch(5).await?;
    println!("✓ Acquired {} agents\n", agents.len());

    // Use them for parallel work
    let tasks: Vec<_> = agents
        .into_iter()
        .enumerate()
        .map(|(i, agent)| {
            tokio::spawn(async move {
                println!("  Agent {} starting work", agent);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                println!("  Agent {} completed work", agent);
                i
            })
        })
        .collect();

    futures::future::join_all(tasks).await;

    println!("\n✓ All batch operations completed");

    Ok(())
}

/// Main example runner
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══════════════════════════════════════════════════════════\n");
    println!("  Agent Object Pool Integration Examples\n");
    println!("═══════════════════════════════════════════════════════════\n");

    // Run examples
    batch_processing_example().await?;
    println!("\n" + &"─".repeat(59) + "\n");

    concurrent_request_example().await?;
    println!("\n" + &"─".repeat(59) + "\n");

    performance_comparison().await?;
    println!("\n" + &"─".repeat(59) + "\n");

    error_handling_example().await?;
    println!("\n" + &"─".repeat(59) + "\n");

    batch_acquisition_example().await?;

    println!("\n" + &"═".repeat(59));
    println!("✅ All examples completed successfully!");
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}

/// Benchmarks for Agent Object Pool Performance
///
/// This benchmark suite evaluates the performance improvements provided by
/// the agent object pool for bulk operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use swarms_rs::agent::pool::{AgentPool, PoolConfig};

/// Simulate agent creation
#[inline]
fn create_mock_agent() -> MockAgent {
    MockAgent {
        id: uuid::Uuid::new_v4().to_string(),
        data: vec![0; 1024], // 1KB of data per agent
    }
}

/// Mock agent for benchmarking
#[derive(Clone)]
struct MockAgent {
    id: String,
    data: Vec<u8>,
}

/// Benchmark: Create agents without pooling
fn benchmark_without_pooling(c: &mut Criterion) {
    c.bench_function("create_100_agents_without_pool", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut agents = Vec::new();
            for _ in 0..100 {
                agents.push(create_mock_agent());
            }
            black_box(agents)
        });
    });
}

/// Benchmark: Create agents with pooling
fn benchmark_with_pooling(c: &mut Criterion) {
    c.bench_function("create_100_agents_with_pool", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let pool = AgentPool::new(100, create_mock_agent);
            let mut agents = Vec::new();
            for _ in 0..100 {
                agents.push(pool.acquire().await.unwrap());
            }
            black_box(agents)
        });
    });
}

/// Benchmark: Varying pool sizes
fn benchmark_pool_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_size_variation");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
                let pool = AgentPool::new(size, create_mock_agent);
                let mut agents = Vec::new();
                for _ in 0..size {
                    agents.push(pool.acquire().await.unwrap());
                }
                black_box(agents)
            });
        });
    }

    group.finish();
}

/// Benchmark: Repeated acquire/release cycles
fn benchmark_acquire_release_cycles(c: &mut Criterion) {
    let mut group = c.benchmark_group("acquire_release_cycles");

    for cycles in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(cycles), cycles, |b, &cycles| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
                let pool = AgentPool::new(50, create_mock_agent);
                for _ in 0..cycles {
                    let agent = pool.acquire().await.unwrap();
                    drop(agent);
                }
            });
        });
    }

    group.finish();
}

/// Benchmark: Batch acquisition
fn benchmark_batch_acquisition(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_acquisition");

    for batch_size in [5, 10, 25, 50].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), batch_size, |b, &batch_size| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
                let pool = AgentPool::new(100, create_mock_agent);
                black_box(pool.acquire_batch(batch_size).await.unwrap())
            });
        });
    }

    group.finish();
}

/// Benchmark: Memory efficiency comparison
fn benchmark_memory_usage(c: &mut Criterion) {
    c.bench_function("memory_usage_with_pool", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let pool = AgentPool::new(100, create_mock_agent);

            // Simulate typical usage pattern
            for _ in 0..10 {
                let batch = pool.acquire_batch(10).await.unwrap();
                for agent in batch {
                    black_box(&agent);
                }
            }
        });
    });
}

/// Benchmark: Contention under concurrent load
fn benchmark_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");

    for num_tasks in [2, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(num_tasks), num_tasks, |b, &num_tasks| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
                let pool = AgentPool::new(50, create_mock_agent);

                let mut handles = Vec::new();
                for _ in 0..num_tasks {
                    let pool_clone = pool.clone();
                    let handle = tokio::spawn(async move {
                        for _ in 0..10 {
                            let _agent = pool_clone.acquire().await.unwrap();
                        }
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    let _ = handle.await;
                }
            });
        });
    }

    group.finish();
}

/// Benchmark: Utilization metrics tracking
fn benchmark_metrics_overhead(c: &mut Criterion) {
    c.bench_function("metrics_collection_overhead", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let config = PoolConfig {
                max_size: 100,
                min_size: 10,
                enable_metrics: true,
                acquire_timeout_ms: 5000,
            };
            let pool = AgentPool::with_config(config, create_mock_agent);

            for _ in 0..100 {
                let _agent = pool.acquire().await.unwrap();
                black_box(pool.metrics());
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_without_pooling,
    benchmark_with_pooling,
    benchmark_pool_sizes,
    benchmark_acquire_release_cycles,
    benchmark_batch_acquisition,
    benchmark_memory_usage,
    benchmark_concurrent_access,
    benchmark_metrics_overhead
);

criterion_main!(benches);

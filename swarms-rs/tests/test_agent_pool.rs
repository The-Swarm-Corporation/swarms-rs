/// Comprehensive tests for Agent Object Pool
///
/// This test module covers all aspects of the agent pooling system including:
/// - Basic pool operations
/// - Concurrent access patterns
/// - Error handling
/// - Metrics tracking
/// - Configuration validation
/// - RAII guard behavior

#[cfg(test)]
mod agent_pool_tests {
    use swarms_rs::agent::pool::{AgentPool, PoolConfig, PoolError, PooledAgent};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Mock agent for testing
    #[derive(Clone)]
    struct TestAgent {
        id: usize,
        value: Arc<AtomicUsize>,
    }

    impl TestAgent {
        fn new(id: usize) -> Self {
            Self {
                id,
                value: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn increment(&self) {
            self.value.fetch_add(1, Ordering::SeqCst);
        }

        fn reset(&self) {
            self.value.store(0, Ordering::SeqCst);
        }
    }

    // ============================================
    // Basic Pool Operation Tests
    // ============================================

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = AgentPool::new(10, || TestAgent::new(1));
        assert_eq!(pool.config.max_size, 10);
    }

    #[tokio::test]
    async fn test_acquire_single_agent() {
        let pool = AgentPool::new(5, || TestAgent::new(1));
        let agent = pool.acquire().await.unwrap();
        assert_eq!(agent.id, 1);
    }

    #[tokio::test]
    async fn test_acquire_multiple_agents() {
        let pool = AgentPool::new(10, || TestAgent::new(1));

        let agents: Vec<_> = (0..5)
            .map(|_| pool.acquire())
            .collect::<Vec<_>>();

        for agent_future in agents {
            let agent = agent_future.await.unwrap();
            assert_eq!(agent.id, 1);
        }
    }

    #[tokio::test]
    async fn test_agent_reuse_from_pool() {
        let creation_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&creation_count);

        let pool = AgentPool::new(5, move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
            TestAgent::new(1)
        });

        // First acquisition creates agent
        let _agent1 = pool.acquire().await.unwrap();
        assert_eq!(creation_count.load(Ordering::SeqCst), 1);

        drop(_agent1);
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Second acquisition reuses agent from pool
        let _agent2 = pool.acquire().await.unwrap();
        assert_eq!(creation_count.load(Ordering::SeqCst), 1); // No new creation
    }

    #[tokio::test]
    async fn test_batch_acquire() {
        let pool = AgentPool::new(20, || TestAgent::new(1));

        let agents = pool.acquire_batch(5).await.unwrap();
        assert_eq!(agents.len(), 5);

        for agent in agents {
            assert_eq!(agent.id, 1);
        }
    }

    #[tokio::test]
    async fn test_batch_acquire_empty() {
        let pool = AgentPool::new(10, || TestAgent::new(1));

        let agents = pool.acquire_batch(0).await.unwrap();
        assert_eq!(agents.len(), 0);
    }

    // ============================================
    // Pool Exhaustion Tests
    // ============================================

    #[tokio::test]
    async fn test_pool_exhaustion_timeout() {
        let config = PoolConfig {
            max_size: 2,
            min_size: 1,
            enable_metrics: true,
            acquire_timeout_ms: 100,
        };

        let pool = AgentPool::with_config(config, || TestAgent::new(1));

        let _agent1 = pool.acquire().await.unwrap();
        let _agent2 = pool.acquire().await.unwrap();

        // Try to acquire more than available
        let result = pool.acquire().await;
        assert!(matches!(result, Err(PoolError::Timeout)));
    }

    #[tokio::test]
    async fn test_pool_recovery_after_release() {
        let config = PoolConfig {
            max_size: 2,
            min_size: 1,
            enable_metrics: true,
            acquire_timeout_ms: 100,
        };

        let pool = AgentPool::with_config(config, || TestAgent::new(1));

        let agent1 = pool.acquire().await.unwrap();
        let agent2 = pool.acquire().await.unwrap();

        // Pool exhausted
        let result = pool.acquire().await;
        assert!(matches!(result, Err(PoolError::Timeout)));

        // Drop one agent to free up space
        drop(agent1);
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Now should be able to acquire again
        let agent3 = pool.acquire().await.unwrap();
        assert_eq!(agent3.id, 1);

        let _ = (agent2, agent3); // Keep them alive
    }

    // ============================================
    // RAII Guard Tests
    // ============================================

    #[tokio::test]
    async fn test_pooled_agent_deref() {
        let pool = AgentPool::new(5, || TestAgent::new(42));

        let agent = pool.acquire().await.unwrap();
        assert_eq!(agent.id, 42);
    }

    #[tokio::test]
    async fn test_pooled_agent_deref_mut() {
        let pool = AgentPool::new(5, || TestAgent::new(1));

        let mut agent = pool.acquire().await.unwrap();
        agent.increment();
        assert_eq!(agent.value.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_pooled_agent_into_agent() {
        let pool = AgentPool::new(5, || TestAgent::new(123));

        let agent_guard = pool.acquire().await.unwrap();
        let agent = agent_guard.into_agent();

        assert_eq!(agent.id, 123);
    }

    // ============================================
    // Concurrent Access Tests
    // ============================================

    #[tokio::test]
    async fn test_concurrent_acquire() {
        let pool = Arc::new(AgentPool::new(50, || TestAgent::new(1)));

        let mut handles = Vec::new();

        for _ in 0..10 {
            let pool_clone = Arc::clone(&pool);
            let handle = tokio::spawn(async move {
                let agent = pool_clone.acquire().await.unwrap();
                agent.increment();
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                agent.value.load(Ordering::SeqCst)
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert_eq!(result, 1); // Each agent should be incremented once
        }
    }

    #[tokio::test]
    async fn test_concurrent_acquire_release() {
        let pool = Arc::new(AgentPool::new(10, || TestAgent::new(1)));

        let mut handles = Vec::new();

        for _ in 0..20 {
            let pool_clone = Arc::clone(&pool);
            let handle = tokio::spawn(async move {
                let agent = pool_clone.acquire().await.unwrap();
                agent.increment();
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                drop(agent);
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }
    }

    // ============================================
    // Configuration Tests
    // ============================================

    #[test]
    fn test_pool_config_validation_success() {
        let config = PoolConfig {
            max_size: 100,
            min_size: 10,
            enable_metrics: true,
            acquire_timeout_ms: 5000,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_pool_config_validation_min_greater_than_max() {
        let config = PoolConfig {
            max_size: 10,
            min_size: 20,
            enable_metrics: true,
            acquire_timeout_ms: 5000,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_pool_config_validation_zero_size() {
        let config = PoolConfig {
            max_size: 0,
            min_size: 0,
            enable_metrics: true,
            acquire_timeout_ms: 5000,
        };

        assert!(config.validate().is_err());
    }

    // ============================================
    // Pool Lifecycle Tests
    // ============================================

    #[tokio::test]
    async fn test_pool_close() {
        let pool = AgentPool::new(5, || TestAgent::new(1));

        pool.close().await;
        assert!(pool.is_closed().await);

        let result = pool.acquire().await;
        assert!(matches!(result, Err(PoolError::Closed)));
    }

    #[tokio::test]
    async fn test_pool_utilization() {
        let config = PoolConfig {
            max_size: 100,
            min_size: 10,
            enable_metrics: true,
            acquire_timeout_ms: 5000,
        };

        let pool = AgentPool::with_config(config, || TestAgent::new(1));

        let _agent = pool.acquire().await.unwrap();
        let utilization = pool.utilization().await;

        assert!(utilization > 0.0 && utilization < 1.0);
    }

    // ============================================
    // Metrics Tests
    // ============================================

    #[tokio::test]
    async fn test_metrics_available_count() {
        let pool = AgentPool::new(5, || TestAgent::new(1));

        let _agent = pool.acquire().await.unwrap();
        let metrics = pool.metrics();

        // After acquiring one agent, available should be less
        assert!(metrics.available_count < 5);
    }

    #[tokio::test]
    async fn test_metrics_in_use_count() {
        let pool = AgentPool::new(5, || TestAgent::new(1));

        let _agent1 = pool.acquire().await.unwrap();
        let _agent2 = pool.acquire().await.unwrap();

        let metrics = pool.metrics();
        assert_eq!(metrics.in_use_count, 2);
    }

    // ============================================
    // Error Handling Tests
    // ============================================

    #[test]
    fn test_pool_error_display() {
        let error = PoolError::PoolFull;
        assert_eq!(error.to_string(), "Pool is full");

        let error = PoolError::PoolExhausted;
        assert_eq!(error.to_string(), "Pool is exhausted");

        let error = PoolError::Closed;
        assert_eq!(error.to_string(), "Pool closed");
    }

    // ============================================
    // Edge Cases
    // ============================================

    #[tokio::test]
    async fn test_pool_with_size_one() {
        let pool = AgentPool::new(1, || TestAgent::new(1));

        let _agent = pool.acquire().await.unwrap();

        // Should timeout when trying to get another
        let result = tokio::time::timeout(
            tokio::time::Duration::from_millis(200),
            pool.acquire(),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_rapid_acquire_release() {
        let pool = AgentPool::new(10, || TestAgent::new(1));

        for _ in 0..100 {
            let agent = pool.acquire().await.unwrap();
            agent.increment();
            drop(agent);
        }

        // Pool should still be functional
        let final_agent = pool.acquire().await.unwrap();
        assert_eq!(final_agent.id, 1);
    }

    #[tokio::test]
    async fn test_pool_clone() {
        let pool1 = AgentPool::new(5, || TestAgent::new(1));
        let pool2 = pool1.clone();

        let agent1 = pool1.acquire().await.unwrap();
        let agent2 = pool2.acquire().await.unwrap();

        assert_eq!(agent1.id, 1);
        assert_eq!(agent2.id, 1);
    }
}

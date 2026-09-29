use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle, available_parallelism};
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

/// A type alias for a function that can be called
pub type Callable<T> = Box<dyn FnOnce() -> T + Send + 'static>;

/// Calculate the optimal number of threads for the current system
pub fn calculate_optimal_threads() -> usize {
    match available_parallelism() {
        Ok(num_cpus) => num_cpus.get(),
        Err(_) => {
            eprintln!("Failed to determine parallelism, defaulting to 4 threads");
            4
        },
    }
}

/// Execute a list of callables in parallel using std::thread
pub fn execute_with_threads<T: Send + 'static>(
    callables: Vec<Callable<T>>,
    num_threads: Option<usize>,
) -> Vec<T> {
    let thread_count = num_threads.unwrap_or_else(calculate_optimal_threads);

    // Create a pool of threads
    let (tx, rx) = std::sync::mpsc::channel();
    let rx = Arc::new(Mutex::new(rx));

    // Store the tasks to be processed
    let tasks: Arc<Mutex<Vec<Callable<T>>>> = Arc::new(Mutex::new(callables));
    let task_count = {
        let tasks = tasks.lock().unwrap();
        tasks.len()
    };

    // Create worker threads
    let mut handles = Vec::with_capacity(thread_count);

    for _ in 0..thread_count {
        let tasks_clone = Arc::clone(&tasks);
        let tx = tx.clone(); // Clone sender for each thread
        let handle = thread::spawn(move || {
            loop {
                // Try to get a task
                let task = {
                    let mut tasks = tasks_clone.lock().unwrap();
                    if tasks.is_empty() {
                        break;
                    }
                    tasks.pop()
                };

                // Execute the task
                if let Some(callable) = task {
                    let result = callable();
                    let _ = tx.send(result);
                }
            }
        });

        handles.push(handle);
    }
    // Drop the original sender to ensure the channel closes when all threads are done
    drop(tx);

    // Collect the results
    let mut results = Vec::with_capacity(task_count);
    {
        let rx_lock = rx.lock().unwrap();
        for result in rx_lock.iter() {
            results.push(result);
        }
    }

    // Wait for all threads to finish
    for handle in handles {
        let _ = handle.join();
    }

    results
}

/// Execute a list of callables in parallel using thread::spawn directly
pub fn execute_with_direct_threads<T: Send + 'static>(callables: Vec<Callable<T>>) -> Vec<T> {
    // Spawn a thread for each callable
    let handles: Vec<JoinHandle<T>> = callables
        .into_iter()
        .map(|callable| thread::spawn(callable))
        .collect();

    // Collect results
    handles
        .into_iter()
        .map(|handle| handle.join().expect("Thread panicked"))
        .collect()
}

/// Custom future for manual async implementation
pub struct CustomFuture<T> {
    shared_state: Arc<Mutex<SharedState<T>>>,
}

struct SharedState<T> {
    completed: bool,
    result: Option<T>,
    waker: Option<Waker>,
}

impl<T> CustomFuture<T> {
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            result: None,
            waker: None,
        }));

        let state_clone = Arc::clone(&shared_state);
        thread::spawn(move || {
            let result = f();
            let mut shared = state_clone.lock().unwrap();
            shared.completed = true;
            shared.result = Some(result);

            if let Some(waker) = shared.waker.take() {
                waker.wake();
            }
        });

        CustomFuture { shared_state }
    }
}

impl<T> Future for CustomFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared = self.shared_state.lock().unwrap();

        if shared.completed {
            Poll::Ready(
                shared
                    .result
                    .take()
                    .expect("Future polled after completion"),
            )
        } else {
            shared.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Execute a list of callables using a custom async runtime
pub async fn execute_with_custom_async<T: Send + 'static>(callables: Vec<Callable<T>>) -> Vec<T> {
    let futures: Vec<CustomFuture<T>> = callables
        .into_iter()
        .map(|callable| CustomFuture::new(callable))
        .collect();

    let mut results = Vec::with_capacity(futures.len());
    for future in futures {
        results.push(future.await);
    }

    results
}

/// Execute a list of callables using Tokio
pub async fn execute_with_tokio<T: Send + 'static>(
    callables: Vec<Callable<T>>,
    _num_threads: Option<usize>,
) -> Vec<T> {
    let mut join_set = JoinSet::new();

    for callable in callables {
        join_set.spawn(async move { callable() });
    }

    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(value) = result {
            results.push(value);
        }
    }

    results
}

/// Execute a list of callables using Tokio with a channel
pub async fn execute_with_tokio_channel<T: Send + 'static>(
    callables: Vec<Callable<T>>,
    num_threads: Option<usize>,
) -> Vec<T> {
    let thread_count = num_threads.unwrap_or_else(calculate_optimal_threads);

    // Create channels
    let (task_tx, task_rx) = mpsc::channel(callables.len());
    let (result_tx, mut result_rx) = mpsc::channel(callables.len());

    // Send all tasks
    for (idx, callable) in callables.into_iter().enumerate() {
        task_tx.send((idx, callable)).await.unwrap();
    }
    drop(task_tx);

    // Spawn workers - since Tokio Receiver can't be cloned, we'll use a multi-producer approach instead
    let task_rx = Arc::new(Mutex::new(task_rx));

    let mut handles = Vec::new();
    for _ in 0..thread_count {
        let task_rx_clone = Arc::clone(&task_rx);
        let result_tx_clone = result_tx.clone();

        let handle = tokio::spawn(async move {
            loop {
                // Try to get a task
                let task_option = {
                    let mut rx_guard = task_rx_clone.lock().unwrap();
                    rx_guard.try_recv().ok()
                };

                match task_option {
                    Some((idx, callable)) => {
                        let result = callable();
                        let _ = result_tx_clone.send((idx, result)).await;
                    },
                    None => {
                        // No more tasks, exit the loop
                        break;
                    },
                }

                // Small delay to reduce lock contention
                tokio::time::sleep(Duration::from_micros(10)).await;
            }
        });

        handles.push(handle);
    }
    drop(result_tx);

    // Collect results
    let mut indexed_results: Vec<(usize, T)> = Vec::new();
    while let Some((idx, result)) = result_rx.recv().await {
        indexed_results.push((idx, result));
    }

    // Wait for all workers to complete
    for handle in handles {
        let _ = handle.await;
    }

    // Sort results by original index
    indexed_results.sort_by_key(|(idx, _)| *idx);
    indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect()
}

/// A master executor that can choose the appropriate concurrency mechanism
pub enum ConcurrencyExecutor {
    StdThreads,
    DirectThreads,
    CustomAsync,
    Tokio,
    TokioChannel,
}

/// Run callables with a specific concurrency mechanism
pub async fn run_concurrent<T: Send + 'static>(
    callables: Vec<Callable<T>>,
    executor: ConcurrencyExecutor,
    num_threads: Option<usize>,
) -> Vec<T> {
    match executor {
        ConcurrencyExecutor::StdThreads => execute_with_threads(callables, num_threads),
        ConcurrencyExecutor::DirectThreads => execute_with_direct_threads(callables),
        ConcurrencyExecutor::CustomAsync => execute_with_custom_async(callables).await,
        ConcurrencyExecutor::Tokio => execute_with_tokio(callables, num_threads).await,
        ConcurrencyExecutor::TokioChannel => {
            execute_with_tokio_channel(callables, num_threads).await
        },
    }
}

/// Run callables with a specific concurrency mechanism (synchronous version)
/// This is useful when you're not in an async context
pub fn run_concurrent_sync<T: Send + 'static>(
    callables: Vec<Callable<T>>,
    executor: ConcurrencyExecutor,
    num_threads: Option<usize>,
) -> Vec<T> {
    match executor {
        ConcurrencyExecutor::StdThreads => execute_with_threads(callables, num_threads),
        ConcurrencyExecutor::DirectThreads => execute_with_direct_threads(callables),
        ConcurrencyExecutor::CustomAsync
        | ConcurrencyExecutor::Tokio
        | ConcurrencyExecutor::TokioChannel => {
            // Create a temporary runtime for executing async code
            let runtime = create_tokio_runtime(num_threads);

            runtime.block_on(async {
                match executor {
                    ConcurrencyExecutor::CustomAsync => execute_with_custom_async(callables).await,
                    ConcurrencyExecutor::Tokio => execute_with_tokio(callables, num_threads).await,
                    ConcurrencyExecutor::TokioChannel => {
                        execute_with_tokio_channel(callables, num_threads).await
                    },
                    _ => unreachable!(),
                }
            })
        },
    }
}

/// Create and configure a Tokio runtime
pub fn create_tokio_runtime(num_threads: Option<usize>) -> Runtime {
    let thread_count = num_threads.unwrap_or_else(calculate_optimal_threads);

    Builder::new_multi_thread()
        .worker_threads(thread_count)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
}

/// Benchmark different concurrency mechanisms
///
/// IMPORTANT: Don't call this from within a tokio::main function or any
/// context where a Tokio runtime is already running, as it will cause a panic.
pub fn benchmark_concurrency<F, T>(
    create_callables: F,
    iterations: usize,
) -> Vec<(String, Duration)>
where
    F: Fn() -> Vec<Callable<T>>,
    T: Send + 'static,
{
    let mut results = Vec::new();

    // Benchmark std::thread pool
    let start = Instant::now();
    for _ in 0..iterations {
        let callables = create_callables();
        let _ = execute_with_threads(callables, None);
    }
    results.push((
        "std::thread pool".to_string(),
        start.elapsed() / iterations as u32,
    ));

    // Benchmark direct threads
    let start = Instant::now();
    for _ in 0..iterations {
        let callables = create_callables();
        let _ = execute_with_direct_threads(callables);
    }
    results.push((
        "direct threads".to_string(),
        start.elapsed() / iterations as u32,
    ));

    // Benchmark Tokio and async methods in a separate function to avoid nested runtime issues
    let tokio_results = benchmark_async_mechanisms(create_callables, iterations);
    results.extend(tokio_results);

    results
}

/// Helper function to benchmark async mechanisms with a fresh runtime
fn benchmark_async_mechanisms<F, T>(
    create_callables: F,
    iterations: usize,
) -> Vec<(String, Duration)>
where
    F: Fn() -> Vec<Callable<T>>,
    T: Send + 'static,
{
    let mut results = Vec::new();

    // Create a dedicated runtime for benchmarking
    let runtime = create_tokio_runtime(None);

    // Benchmark Tokio
    let start = Instant::now();
    for _ in 0..iterations {
        let callables = create_callables();
        let _ = runtime.block_on(execute_with_tokio(callables, None));
    }
    results.push(("Tokio".to_string(), start.elapsed() / iterations as u32));

    // Benchmark Tokio with channel
    let start = Instant::now();
    for _ in 0..iterations {
        let callables = create_callables();
        let _ = runtime.block_on(execute_with_tokio_channel(callables, None));
    }
    results.push((
        "Tokio with channel".to_string(),
        start.elapsed() / iterations as u32,
    ));

    // Benchmark custom async
    let start = Instant::now();
    for _ in 0..iterations {
        let callables = create_callables();
        let _ = runtime.block_on(execute_with_custom_async(callables));
    }
    results.push((
        "custom async".to_string(),
        start.elapsed() / iterations as u32,
    ));

    results
}

// Example usage
#[tokio::main]
async fn main() {
    // Create example callables that can be used once
    let create_callables = || {
        let mut callables: Vec<Callable<i32>> = Vec::new();
        for i in 0..10 {
            let callable: Callable<i32> = Box::new(move || {
                // Simulate work
                thread::sleep(Duration::from_millis(100));
                i
            });
            callables.push(callable);
        }
        callables
    };

    // Execute with different mechanisms
    println!("Using std::thread pool:");
    let results = execute_with_threads(create_callables(), None);
    println!("Results: {:?}", results);

    println!("\nUsing direct threads:");
    let results = execute_with_direct_threads(create_callables());
    println!("Results: {:?}", results);

    println!("\nUsing Tokio:");
    let results = execute_with_tokio(create_callables(), None).await;
    println!("Results: {:?}", results);

    println!("\nUsing Tokio with channel:");
    let results = execute_with_tokio_channel(create_callables(), None).await;
    println!("Results: {:?}", results);

    println!("\nUsing custom async:");
    let results = execute_with_custom_async(create_callables()).await;
    println!("Results: {:?}", results);

    // Note: We don't run benchmarking inside the tokio::main function
    // because it would cause a "Cannot start a runtime from within a runtime" panic
    println!("\nSkipping benchmarks when running with #[tokio::main]");
    println!("To run benchmarks, use the benchmark_concurrency function in a separate binary");
}

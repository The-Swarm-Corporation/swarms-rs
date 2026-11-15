//! Tests for how `OpenAI` builds and shares its HTTP client.
//!
//! `OpenAI::from_url` hands async-openai one `reqwest::Client` (built by `build_http_client`)
//! instead of letting `Client::with_config` build a default one first and discard it. These
//! tests pin down what must not change with that: the client that is used, default retry/backoff,
//! credential / endpoint / header isolation between instances, and behaviour across clones,
//! concurrency, cancellation, failures and Tokio runtimes.
//!
//! Everything runs against a local keep-alive mock server, so no API key or network is needed.
//! The server runs on plain threads, not on a test runtime, so dropping a runtime never stops it.
//!
//! Separately built instances must never share a client: pooled connections belong to the runtime
//! that opened them, and sharing one client between independent runtimes that run at the same
//! time can fail with `DispatchGone` or never complete. A model may be cloned and shared within
//! one runtime (see `clones_*`).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use swarms_rs::llm::Model;
use swarms_rs::llm::completion::Message;
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::llm::request::CompletionRequest;

type Headers = Vec<(String, String)>;

struct Seen {
    headers: Headers,
    body: String,
}

#[derive(Default)]
struct State {
    connections: AtomicUsize,
    requests: Mutex<Vec<Seen>>,
    /// Hold each response back this long (milliseconds).
    delay_ms: AtomicU64,
    /// Read the next request, then close the connection without answering.
    close_next: AtomicBool,
    /// Status codes to answer the next requests with (error body), before going back to 200.
    statuses: Mutex<std::collections::VecDeque<u16>>,
}

struct Mock {
    base_url: String,
    state: Arc<State>,
}

impl Mock {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(State::default());
        let s = state.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                s.connections.fetch_add(1, Ordering::SeqCst);
                let s = s.clone();
                std::thread::spawn(move || serve(stream, s));
            }
        });
        Mock {
            base_url: format!("http://{addr}/v1"),
            state,
        }
    }

    fn connections(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }

    fn request_count(&self) -> usize {
        self.state.requests.lock().unwrap().len()
    }

    fn header(&self, request: usize, name: &str) -> Option<String> {
        self.state.requests.lock().unwrap()[request]
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }

    fn body(&self, request: usize) -> String {
        self.state.requests.lock().unwrap()[request].body.clone()
    }
}

/// Answer every request on one connection until the client closes it (HTTP/1.1 keep-alive).
fn serve(stream: TcpStream, state: Arc<State>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let body = json!({
        "id": "x", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"},
                     "finish_reason": "stop"}]
    })
    .to_string();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let (mut headers, mut content_length) = (Headers::new(), 0);
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).unwrap_or(0) == 0 {
                return;
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                if k.eq_ignore_ascii_case("content-length") {
                    content_length = v.trim().parse().unwrap_or(0);
                }
                headers.push((k.to_string(), v.trim().to_string()));
            }
        }
        let mut payload = vec![0; content_length];
        if reader.read_exact(&mut payload).is_err() {
            return;
        }
        state.requests.lock().unwrap().push(Seen {
            headers,
            body: String::from_utf8_lossy(&payload).into_owned(),
        });
        if state.close_next.swap(false, Ordering::SeqCst) {
            return; // drops the socket mid-request
        }
        let delay = state.delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        let (code, payload) = match state.statuses.lock().unwrap().pop_front() {
            Some(code) => (
                code,
                r#"{"error":{"message":"scripted","type":"requests","param":null,"code":null}}"#
                    .to_string(),
            ),
            None => (200, body.clone()),
        };
        let response = format!(
            "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        );
        if writer.write_all(response.as_bytes()).is_err() {
            return;
        }
    }
}

fn request_with(prompt: &str) -> CompletionRequest {
    CompletionRequest {
        prompt: Message::user(prompt),
        system_prompt: None,
        chat_history: vec![],
        tools: vec![],
        temperature: None,
        max_tokens: Some(8),
    }
}

fn request() -> CompletionRequest {
    request_with("hi")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// One completion on a brand-new runtime, which is dropped (with its tasks) on return.
fn complete_on_fresh_runtime<M: Model>(model: &M) -> Result<(), String> {
    // Bounded, so a lost connection shows up as a failure instead of hanging the test run
    // (reqwest has no default timeout).
    runtime()
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(10), model.completion(request())).await
        })
        .map_err(|_| "no response within 10 s".to_string())?
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ---- behavior kept when the client is built once (`Client::build`) -------------------------

/// `OpenAI::from_url` hands async-openai the client from `build_http_client` directly instead of
/// letting `Client::with_config` build a default one first. A default `reqwest::Client` sends no
/// User-Agent, so this fails if the wrong client ends up in use.
#[test]
fn requests_use_the_configured_http_client() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    runtime().block_on(model.completion(request())).unwrap();
    assert_eq!(mock.header(0, "user-agent").as_deref(), Some("swarms-rs"));
}

/// async-openai retries rate-limited (429) and 5xx replies with exponential backoff. The
/// backoff is the default one `with_config` used, passed explicitly to `Client::build`.
#[test]
fn rate_limited_and_server_error_replies_are_retried() {
    let mock = Mock::start();
    mock.state.statuses.lock().unwrap().extend([429, 500]);
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    runtime()
        .block_on(model.completion(request()))
        .expect("retry with the default backoff should reach the 200");
    assert_eq!(mock.request_count(), 3);
}

/// Cancelling while a retry is pending must not leave the client unusable.
#[test]
fn cancelling_during_retry_backoff_leaves_the_client_usable() {
    let mock = Mock::start();
    mock.state.statuses.lock().unwrap().extend([429; 50]);
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    rt.block_on(async {
        let r = tokio::time::timeout(
            Duration::from_millis(300),
            model.clone().completion(request()),
        )
        .await;
        assert!(r.is_err(), "still retrying, so the timeout should fire");
    });
    mock.state.statuses.lock().unwrap().clear();
    rt.block_on(model.clone().completion(request())).unwrap();
}

// ---- cloning shares one client ------------------------------------------------------------

#[test]
fn clones_reuse_one_connection() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    const N: usize = 20;
    for _ in 0..N {
        rt.block_on(model.clone().completion(request())).unwrap();
    }
    // One client per clone would open N connections; a shared client reuses its idle one.
    assert!(
        mock.connections() < N,
        "{} connections for {N} clones",
        mock.connections()
    );
}

#[test]
fn clones_run_concurrently_in_one_multi_thread_runtime() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let tasks: Vec<_> = (0..100)
            .map(|_| {
                let m = model.clone();
                tokio::spawn(async move { m.completion(request()).await.map(|_| ()) })
            })
            .collect();
        for t in tasks {
            t.await.unwrap().expect("completion failed");
        }
    });
    assert_eq!(mock.request_count(), 100);
}

#[test]
fn clones_survive_sequential_runtimes() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    // Each call runs on a new runtime; the previous one is gone, leaving an idle pooled
    // connection whose driver task was dropped with it.
    for _ in 0..50 {
        complete_on_fresh_runtime(&model.clone()).expect("completion failed");
    }
}

// ---- separate instances stay isolated -----------------------------------------------------

#[test]
fn separate_instances_do_not_share_connections() {
    let mock = Mock::start();
    let rt = runtime();
    const N: usize = 10;
    for i in 0..N {
        let model = OpenAI::from_url(mock.base_url.clone(), format!("key-{i}"));
        rt.block_on(model.completion(request())).unwrap();
    }
    assert_eq!(mock.connections(), N);
}

#[test]
fn each_instance_keeps_its_own_credentials() {
    let mock = Mock::start();
    let rt = runtime();
    for i in 0..10 {
        let model = OpenAI::from_url(mock.base_url.clone(), format!("key-{i}"));
        rt.block_on(model.completion(request())).unwrap();
    }
    for i in 0..10 {
        assert_eq!(
            mock.header(i, "authorization").as_deref(),
            Some(format!("Bearer key-{i}").as_str())
        );
    }
}

#[test]
fn each_instance_keeps_its_own_endpoint() {
    let (a, b) = (Mock::start(), Mock::start());
    let rt = runtime();
    let (ma, mb) = (
        OpenAI::from_url(a.base_url.clone(), "key-a".to_string()),
        OpenAI::from_url(b.base_url.clone(), "key-b".to_string()),
    );
    for _ in 0..3 {
        rt.block_on(ma.completion(request())).unwrap();
        rt.block_on(mb.completion(request())).unwrap();
    }
    assert_eq!((a.request_count(), b.request_count()), (3, 3));
    assert_eq!(
        a.header(2, "authorization").as_deref(),
        Some("Bearer key-a")
    );
    assert_eq!(
        b.header(2, "authorization").as_deref(),
        Some("Bearer key-b")
    );
}

#[test]
fn custom_headers_do_not_reach_other_instances() {
    let mock = Mock::start();
    let routed = OpenRouter::from_url(mock.base_url.clone(), "k".to_string())
        .with_app_url("https://example.com")
        .with_app_name("tests");
    let before = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let after = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    rt.block_on(before.completion(request())).unwrap();
    rt.block_on(routed.completion(request())).unwrap();
    rt.block_on(after.completion(request())).unwrap();

    assert_eq!(mock.header(0, "http-referer"), None);
    assert_eq!(
        mock.header(1, "http-referer").as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        mock.header(1, "x-openrouter-title").as_deref(),
        Some("tests")
    );
    assert_eq!(mock.header(2, "http-referer"), None);
    assert_eq!(mock.header(2, "x-openrouter-title"), None);
}

#[test]
fn requests_on_a_shared_client_carry_no_earlier_request_state() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    for i in 0..5 {
        rt.block_on(
            model
                .clone()
                .completion(request_with(&format!("secret-prompt-{i}"))),
        )
        .unwrap();
    }
    for i in 0..5 {
        let body = mock.body(i);
        assert!(body.contains(&format!("secret-prompt-{i}")));
        for j in (0..5).filter(|j| *j != i) {
            assert!(
                !body.contains(&format!("secret-prompt-{j}")),
                "request {i} leaked prompt {j}"
            );
        }
    }
}

/// Guard against a process-wide cached client. Every worker has its own runtime, torn down
/// while others run, and builds fresh instances; this must never fail. With a cached client it
/// failed ~7% of runs with `DispatchGone: runtime dropped the dispatch task`.
#[test]
fn separate_instances_work_across_concurrent_independent_runtimes() {
    let mock = Mock::start();
    let base = mock.base_url.clone();
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let base = base.clone();
            std::thread::spawn(move || {
                for i in 0..25 {
                    let model = OpenAI::from_url(base.clone(), format!("k{t}-{i}"));
                    complete_on_fresh_runtime(&model).expect("completion failed");
                }
            })
        })
        .collect();
    for h in handles {
        h.join().expect("worker panicked");
    }
    assert_eq!(mock.request_count(), 8 * 25);
}

// ---- failure handling on a shared client ---------------------------------------------------

#[test]
fn a_timed_out_request_does_not_poison_the_shared_client() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    rt.block_on(async {
        mock.state.delay_ms.store(3000, Ordering::SeqCst);
        let slow = tokio::time::timeout(
            Duration::from_millis(100),
            model.clone().completion(request()),
        )
        .await;
        assert!(slow.is_err(), "expected the 100 ms timeout to fire");

        mock.state.delay_ms.store(0, Ordering::SeqCst);
        model
            .clone()
            .completion(request())
            .await
            .expect("client unusable after a cancelled request");
    });
}

#[test]
fn a_dropped_request_future_does_not_poison_the_shared_client() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    rt.block_on(async {
        mock.state.delay_ms.store(3000, Ordering::SeqCst);
        let handle = tokio::spawn({
            let m = model.clone();
            async move { m.completion(request()).await.map(|_| ()) }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.abort(); // cancellation while the request is in flight
        let _ = handle.await;

        mock.state.delay_ms.store(0, Ordering::SeqCst);
        model.clone().completion(request()).await.unwrap();
    });
}

#[test]
fn connection_refused_is_an_error_and_the_client_recovers() {
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}/v1", l.local_addr().unwrap()) // listener dropped: port is closed
    };
    let rt = runtime();
    let model = OpenAI::from_url(dead, "k".to_string());
    assert!(rt.block_on(model.clone().completion(request())).is_err());
    assert!(rt.block_on(model.clone().completion(request())).is_err());

    let mock = Mock::start();
    let ok = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    rt.block_on(ok.completion(request())).unwrap();
}

#[test]
fn a_connection_closed_mid_request_is_an_error_and_the_client_recovers() {
    let mock = Mock::start();
    let model = OpenAI::from_url(mock.base_url.clone(), "k".to_string());
    let rt = runtime();
    rt.block_on(model.clone().completion(request())).unwrap(); // pool one idle connection
    mock.state.close_next.store(true, Ordering::SeqCst);
    assert!(rt.block_on(model.clone().completion(request())).is_err());
    rt.block_on(model.clone().completion(request()))
        .expect("client did not recover after the server dropped a connection");
}

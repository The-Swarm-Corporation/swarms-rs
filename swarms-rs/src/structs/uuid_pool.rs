use std::collections::VecDeque;
use std::sync::{Arc, Mutex, LazyLock};
use std::thread;
use uuid::Uuid;

// Public type
pub struct IdPool {
    queue: VecDeque<String>,
    max_size: usize,
    min_threshold: usize,
}

impl IdPool {
    pub fn new(max_size: usize, min_threshold: usize) -> Self {
        let mut pool = IdPool {
            queue: VecDeque::with_capacity(max_size),
            max_size,
            min_threshold,
        };
        pool.refill();
        pool
    }

    fn refill(&mut self) {
        while self.queue.len() < self.max_size {
            self.queue.push_back(Uuid::new_v4().to_string());
        }
    }

    fn get_id_inner(&mut self) -> String {
        if let Some(id) = self.queue.pop_front() {
            if self.queue.len() < self.min_threshold {
                Self::schedule_refill();
            }
            id
        } else {
            let id = Uuid::new_v4().to_string();
            Self::schedule_refill();
            id
        }
    }

    fn schedule_refill() {
        thread::spawn(|| {
            if let Ok(mut pool) = GLOBAL_ID_POOL.lock() {
                pool.refill();
            }
        });
    }

    pub fn get_uuid() -> String {
        GLOBAL_ID_POOL
            .lock()
            .map(|mut pool| pool.get_id_inner())
            .unwrap_or_else(|_| Uuid::new_v4().to_string())
    }
}

pub static GLOBAL_ID_POOL: LazyLock<Arc<Mutex<IdPool>>> = LazyLock::new(|| {
    Arc::new(Mutex::new(IdPool::new(
        100, // max size
        5,   // refill threshold
    )))
});

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde_json::Value;

struct Entry {
    expires: Instant,
    value: Arc<Value>,
}

/// Tiny in-memory TTL cache. The upstream site is undocumented and rate-limit
/// behaviour is unknown, so every read goes through here first.
pub struct Cache {
    inner: Mutex<HashMap<String, Entry>>,
    max_entries: usize,
}

impl Cache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max_entries,
        }
    }

    pub fn get(&self, key: &str) -> Option<Arc<Value>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.get(key)?;
        if entry.expires > Instant::now() {
            Some(entry.value.clone())
        } else {
            None
        }
    }

    pub fn set(&self, key: String, value: Arc<Value>, ttl: Duration) {
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        guard.retain(|_, e| e.expires > Instant::now());
        if guard.len() >= self.max_entries {
            guard.clear();
        }
        guard.insert(
            key,
            Entry {
                expires: Instant::now() + ttl,
                value,
            },
        );
    }
}

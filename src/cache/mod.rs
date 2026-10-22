use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

struct Entries<K, V> {
    values: HashMap<K, (V, usize)>,
    order: VecDeque<K>,
    bytes: usize,
}

pub struct Cache<K, V> {
    entries: Arc<Mutex<Entries<K, V>>>,
    capacity: usize,
}

impl<K, V> Clone for Cache<K, V> {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            capacity: self.capacity,
        }
    }
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self::with_capacity(64 * 1024 * 1024)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Entries {
                values: HashMap::new(),
                order: VecDeque::new(),
                bytes: 0,
            })),
            capacity,
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let value = entries.values.get(key).map(|(value, _)| value.clone());
        if value.is_some() {
            entries.order.retain(|entry| entry != key);
            entries.order.push_back(key.clone());
        }
        value
    }

    pub fn insert(&self, key: K, value: V) {
        self.insert_weighted(key, value, 1);
    }

    pub fn insert_weighted(&self, key: K, value: V, weight: usize) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((_, old_weight)) = entries.values.remove(&key) {
            entries.bytes -= old_weight;
        }
        entries.order.retain(|entry| entry != &key);
        if weight > self.capacity {
            return;
        }
        while entries.bytes + weight > self.capacity {
            if let Some(old_key) = entries.order.pop_front() {
                if let Some((_, old_weight)) = entries.values.remove(&old_key) {
                    entries.bytes -= old_weight;
                }
            } else {
                break;
            }
        }
        entries.bytes += weight;
        entries.order.push_back(key.clone());
        entries.values.insert(key, (value, weight));
    }

    pub fn invalidate(&self, key: &K) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((_, weight)) = entries.values.remove(key) {
            entries.bytes -= weight;
        }
        entries.order.retain(|entry| entry != key);
    }

    pub fn invalidate_where(&self, predicate: impl Fn(&K) -> bool) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        entries.values.retain(|key, _| !predicate(key));
        entries.order.retain(|key| !predicate(key));
        entries.bytes = entries.values.values().map(|(_, weight)| weight).sum();
    }
}

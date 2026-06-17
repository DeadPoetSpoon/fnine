use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

/// Simple in-memory cache with optional capacity limit.
pub struct Cache<K, V> {
    map: Arc<Mutex<HashMap<K, V>>>,
}

impl<K, V> Clone for Cache<K, V> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.map.lock().unwrap().get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) {
        self.map.lock().unwrap().insert(key, value);
    }

    pub fn invalidate(&self, key: &K) {
        self.map.lock().unwrap().remove(key);
    }
}

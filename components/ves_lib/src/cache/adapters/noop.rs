use crate::cache::traits::Cache;

use std::hash::Hash;

/// No-op cache. This is always a miss, never actually storing anything.
/// This is intended for use in tests to disable caching without changing call
/// sites
#[derive(Default)]
pub struct NoopCache;

impl NoopCache {
    pub fn new() -> Self {
        Self
    }
}

impl<K, V> Cache<K, V> for NoopCache
where
    K: Eq + Hash + Send + Sync,
    V: Clone + Send + Sync,
{
    fn insert(&self, _key: K, _value: V) {}

    fn get(&self, _key: &K) -> Option<V> {
        None
    }
    
    fn remove(&self, _key: &K) {}
    
    fn len(&self) -> usize {
        0
    }
}    
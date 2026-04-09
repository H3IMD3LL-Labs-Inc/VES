use crate::cache::traits::{
    cache::Cache,
    policy::CachePolicy,
};

use std::hash::Hash;

const DEFAULT_CAPACITY: usize = 10_000;

pub struct QuickCacheAdapter<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    inner: quick_cache::sync::Cache<K, V>
}

impl<K, V> QuickCacheAdapter<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    pub fn new(policy: Option<CachePolicy>) -> Self {
        // quick_cache does not support TTL, it is a pure capacity-bounded
        // LRU cache. TTL fields in CachePolicy are intentionally ignored here.
        // Use MokaCacheAdapter if TTL eviction is required in the cache.
        let capacity = policy
            .as_ref()
            .and_then(|p| p.capacity)
            .unwrap_or(DEFAULT_CAPACITY);

        Self {
            inner: quick_cache::sync::Cache::new(capacity),
        }
    }
}

impl<K, V> Cache<K, V> for QuickCacheAdapter<K, V>
where
    K: Eq + Hash + Send + Sync + Clone + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key)
    }

    fn insert(&self, key: K, value: V) {
        self.inner.insert(key, value);
    }

    fn remove(&self, key: &K) {
        self.inner.len()
    }
}
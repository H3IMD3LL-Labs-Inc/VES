use crate::cache::traits::{
    cache::Cache,
    policy::{
        CachePolicy,
        CacheTtl,
        EvictionStrategy
    },
};

use std::hash::Hash;

pub struct MokaCacheAdapter<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    inner: moka::sync::Cache<K, V>
}

impl<K, V> MokaCacheAdapter<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(policy: Option<CachePolicy>) -> Self {
        let mut builder = moka::sync::Cache::builder();

        if let Some(policy) = policy {
            if let Some(capacity) = policy.capacity {
                builder = builder.max_capacity(capacity as u64);
            }
            if let Some(ttl) = policy.ttl {
                match ttl {
                    CacheTtl::Fixed(duration) => {
                        builder = builder.time_to_live(duration);
                    }
                    CacheTtl::Sliding(duration) => {
                        /// In Moka, time_to_idle resets on read/write access on the cache
                        builder = builder.time_to_idle(duration);
                    }
                }
            }
            match policy.eviction {
                /// Moka uses LRU/LFU internally via the TinyLFU policy, no explicit
                /// eviction strategy toggle is needed
                EvictionStrategy::Lru | EvictionStrategy::Lfu => {}
            }
        }

        Self { inner: builder.build() }
    }
}

impl<K, V> Cache<K, V> for MokaCacheAdapter<K, V>
where
    K: Eq + Hash + Send + Sync + Clone + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn get(&self, key: &K) -> Option<V> {
        self.inner.get(&key)
    }

    fn insert(&self, key: K, value: V) {
        self.inner.insert(key, value);
    }

    fn remove(&self, key: &K) {
        self.inner.invalidate(key);
    }

    fn len(&self) -> usize {
        self.inner.entry_count as usize
    }
}
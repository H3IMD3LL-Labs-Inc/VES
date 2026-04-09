use crate::cache::{
    errors::CacheError,
    metrics::cache_metrics::{
        CacheMetrics,
        CacheMetricsSnapshot,
    },
    traits::cache::Cache,
};

use std::hash::Hash;
use std::sync::Arc;

/// A cache wrapper that transparently records metrics on every operation.
/// Subsystems opt in by wrapping their cache with this, it implements
/// `Cache<K, V>` so it's a drop-in replacement at any call site.
///
/// # Example
/// ```rust
/// let inner = CacheBuilder::<String, String>::new()
///     .backend(CacheBackend::QuickCache)
///     .policy(CachePolicy::new(1_000)
///     .build()
///
/// let cache = InstrumentedCache::new(inner);
/// ```
pub struct InstrumentedCache<K, V> {
    inner: Arc<dyn Cache<K, V>>,
    metrics: Arc<CacheMetrics>,
}

impl<K, V> InstrumentedCache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Wrap any `SharedCache` with instrumentation
    pub fn new(inner: Arc<dyn Cache<K, V>>) -> Self {
        Self {
            inner,
            metrics: Arc::new(CacheMetrics::new()),
        }
    }

    /// Returns a snapshot of current metrics for this cache instance
    pub fn snapshot(&self) -> CacheMetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Returns a cloned handle to the underlying `CacheMetrics`.
    /// Use this if you want to hold a reference to live metrics
    /// rather than taking periodic snapshots
    pub fn metrics(&self) -> Arc<CacheMetrics> {
        Arc::clone(&self.metrics)
    }
}

impl<K, V> Cache<K, V> for InstrumentedCache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn insert(&self, key: K, value: V) -> Result<(), CacheError> {
        let result = self.inner.insert(key, value);
        if result.is_ok() {
            self.metrics.record_insertion();
        }
        result
    }

    fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        let result = self.inner.get(key)?;
        match &result {
            Some(_) => self.metrics.record_hit(),
            None => self.metrics.record_miss(),
        }
        Ok(result)
    }

    fn remove(&self, key: &K) {
        self.inner.remove(key);
    }

    fn clear(&self) -> Result<(), CacheError> {
        self.inner.clear()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}
use crate::cache::errors::CacheError;

use std::hash::Hash;

/// Core cache trait. All cache backends must implement this.
/// Caches are ephemeral, never use for correctness-critical states.
/// Prefer "persistence to disk" for these
pub trait Cache<K, V>: Send + Sync
where
    K: Eq + Hash + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Insert a key-value pair into the cache
    fn insert(&self, key: K, value: V) -> Result<(), CacheError>;

    /// Retrieve a value by key. Returns `None` if not present or evicted
    fn get(&self, key: &K) -> Result<Option<V>, CacheError>;

    /// Remove a key from the cache explicitly. Returns nothing, since some
    /// backends DO NOT support returning the evicted value(e.g., moka)
    fn remove(&self, key: &K);

    /// Remove all entries from the cache
    fn clear(&self) -> Result<(), CacheError> {
        Err(CacheError::OperationUnsupported(
            "QuickCache does not support clear(); drop and recreate the instance". into(),
        ))
    }

    /// Returns the number of entries currently in the cache
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
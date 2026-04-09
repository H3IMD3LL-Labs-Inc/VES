/// This is purely a DX layer, not core logic. It helps avoid repetitive generics
/// across the VES Platform components
use std::hash::Hash;
use std::sync::Arc;

use super::cache::Cache;
use crate::cache::metrics::instrument::InstrumentedCache;

/// A shared, instrumented cache. This is preferred when running the VES Platform
/// in production and with `self-observability` needs
pub type InstrumentedSharedCache<K, V> = Arc<InstrumentedCache<K, V>>;

/// A boxed, owned cache for single-owner subsystems. Use when a subsystem owns
/// its cache exclusively.
///
/// For example; `DynCache<String, ParsedResult>`
pub type DynCache<K, V> = Box<dyn Cache<K, V>>;

/// A thread-safe shared cache for cases where multiple parts of a subsystem need
/// access to the same cache instance. Use this when you need to clone a handle and
/// share it across tasks.
///
/// For example; `SharedCache<RequestId, ResponsePayload>`
pub type SharedCache<K, V> = Arc<dyn Cache<K, V>>;

/// Convenience constructor for a `SharedCache` from any `Cache` impl.
pub fn shared<K, V, C>(cache: C) -> SharedCache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    C: Cache<K, V> + 'static,
{
    Arc::new(cache)
}

/// Convenience constructor for `DynCache` from any `Cache` impl.
pub fn dynamic<K, V, C>(cache: C) -> DynCache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    C: Cache<K, V> + 'static,
{
    Box::new(cache)
}
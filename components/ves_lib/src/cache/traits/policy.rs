use std::time::Duration;

/// Full cache policy for a single cache instance. Passed into adapter
/// constructors to configure behavior.
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Time-based expiration strategy, if any
    pub ttl: Option<CacheTtl>,
    /// Maximum number of entries. None means unbounded (P.S, use carefully)
    pub capacity: Option<usize>,
    /// Eviction strategy when capacity is exceeded
    pub eviction: EvictionStrategy
}

impl CachePolicy {
    /// Sensible default: LRU, no TTL, caller must set capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: Some(capacity),
            ttl: None,
            eviction: EvictionStrategy::Lru,
        }
    }

    /// Convenience method: LRU + fixed TTL
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity: Some(capacity),
            ttl: Some(CacheTtl::Fixed(ttl)),
            eviction: EvictionStrategy::Lru,
        }
    }
}

/// Time-based expiration strategy
#[derive(Debug, Clone)]
pub enum CacheTtl {
    /// Entry expires after a fixed time-to-live duration from insertion
    Fixed(Duration),
    /// Entry expiration resets on each access
    Sliding(Duration),
}

/// Eviction strategy when the cache reaches capacity
#[derive(Debug, Clone, Default)]
pub enum EvictionStrategy {
    /// Least Recently Used, good default for most subsystems
    #[default]
    Lru,
    /// Least Frequently Used, should be better for the HEIMDELL Server's access patterns
    Lfu,
}
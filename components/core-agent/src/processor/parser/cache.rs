use ves_lib::cache::{
    builder::{CacheBuilder, CacheBackend},
    errors::CacheError,
    metrics::InstrumentedCache,
    traits::{
        cache::Cache,
        policy::CachePolicy,
        typed::SharedCache,
    },
};
use super::types::LogEvent;

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParserCacheKey(u64);

impl ParserCacheKey {
    pub fn from_raw(raw: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        raw.hash(&mut hasher);
        Self(hasher.finish())
    }
}

#[derive(Clone)]
pub struct CachedParseResult {
    pub event: LogEvent,
}

pub type ParserCache = Arc<InstrumentedCache<ParserCacheKey, CachedParseResult>>;

pub fn build_parser_cache() -> ParserCache {
    /// [TODO]: Should this capacity of 5_000 be tunable?? (*intense thinking noises)
    let policy = CachePolicy::new(5_000);

    let inner: SharedCache<ParserCacheKey, CachedParseResult> = CacheBuilder::new()
        .backend(CacheBackend::QuickCache)
        .policy(policy)
        .build();

    Arc::new(InstrumentedCache::new(inner))
}


use crate::cache::{
    adapters::NoopCache,
    traits::{
        cache::Cache,
        policy::CachePolicy,
        typed::SharedCache,
    },
};

use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

#[cfg(feature = "moka")]
use crate::cache::adapters::MokaCacheAdapter;

#[cfg(feature = "quick_cache")]
use crate::cache::adapters::QuickCacheAdapter;

#[derive(Debug, Clone)]
pub enum CacheBackend {
    #[cfg(feature = "quick_cache")]
    QuickCache,
    #[cfg(feature = "moka")]
    Moka,
    Noop,
}

pub struct CacheBuilder<K, V> {
    backend: CacheBackend,
    policy: Option<CachePolicy>,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> CacheBuilder<K, V> {
    pub fn new() -> Self {
        Self {
            backend: CacheBackend::Noop,
            policy: None,
            _marker: PhantomData,
        }
    }

    pub fn backend(mut self, backend: CacheBackend) -> Self {
        self.backend = backend;
        self
    }

    pub fn policy(mut self, policy: CachePolicy) -> Self {
        self.policy = Some(policy);
        self
    }
}

impl<K, V> CacheBuilder<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + Send + Sync + 'static,
{
    pub fn build(self) -> SharedCache<K, V> {
        match self.backend {
            #[cfg(feature = "quick_cache")]
            CacheBackend::QuickCache => Arc::new(QuickCacheAdapter::new(self.policy)),

            #[cfg(feature = "moka")]
            CacheBackend::Moka => Arc::new(MokaCacheAdapter::new(self.policy)),

            CacheBackend::Noop => Arc::new(NoopCache::new()),
        }
    }
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
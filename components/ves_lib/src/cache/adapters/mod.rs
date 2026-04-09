#[cfg(feature = "moka")]
pub mod moka;

#[cfg(feature = "quick_cache")]
pub mod quick_cache;

pub mod noop;

#[cfg(feature = "moka")]
pub use moka::MokaCacheAdapter;

#[cfg(feature = "quick_cache")]
pub use quick_cache::QuickCacheAdapter;

pub use noop::NoopCache;
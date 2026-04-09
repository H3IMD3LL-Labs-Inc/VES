pub mod cache;
pub mod policy;
pub mod typed;

pub use cache::Cache;
pub use policy::{CachePolicy, EvictionStrategy};
pub use typed::{DynCache, SharedCache};
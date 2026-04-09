use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    /// Returned when a cache insertion fails at the backend level
    #[error("Failed to insert key into cache")]
    InsertionOperationFailed,

    ///Returned when a cache lookup fails at the backend level,
    /// Distinguished from a cache miss which is Ok(None), this is an
    /// actual backend failure
    #[error("Cache lookup failed: {0}")]
    LookupOperationFailed(String),

    /// Returned when the cache backend is in an unrecoverable state
    #[error("Cache backend is unavailable")]
    BackendUnavailable,

    /// Returned when an operation is not supported by the backend
    #[error("Cache operation rejected: {0}")]
    OperationUnsupported(String),
}
// Local crate
use crate::{
    helpers::load_config::{BufferConfig, DurabilityConfig},
    parser::parser::NormalizedLog,
};

// External crates
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Result, params};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::Instant;
use tracing::instrument;

/// In-Memory Buffer (runtime structure)
#[derive(Debug, Clone)]
pub struct InMemoryBuffer {
    pub queue: VecDeque<NormalizedLog>,
    pub buffer_capacity: u64,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub last_flush_at: Instant,
    pub durability: Durability,
    pub overflow_policy: String,
    pub drain_policy: String,
    pub flush_policy: String,
    pub notify: Arc<Notify>,
}

/// Runtime durability (contains real durability resources used by InMemoryBuffer)
#[derive(Debug, Clone)]
pub enum Durability {
    InMemory,
    SQLite(Arc<Pool<SqliteConnectionManager>>),
}

impl InMemoryBuffer {
    #[instrument(
        name = "ves_inmemory_buffer_create",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    pub async fn new(buffer_config: BufferConfig) -> Result<Self, String> {
        tracing::debug!(
            in_memory_buffer_configuration = ?buffer_config,
            "Creating new InMemoryBuffer using buffer configurations"
        );

        let queue = match buffer_config.capacity_option.as_str() {
            "bounded" => {
                tracing::info!(
                    bounded_inmemory_buffer_capacity = ?buffer_config.buffer_capacity,
                    "Creating bounded InMemoryBuffer with set capacity"
                );
                VecDeque::with_capacity(buffer_config.buffer_capacity as usize)
            }
            "unbounded" => {
                tracing::warn!("Creating unbounded InMemoryBuffer without set capacity");
                VecDeque::new()
            }
            other => {
                tracing::error!(
                    inmemory_buffer_capacity = ?other,
                    "Invalid InMemoryBuffer buffer capacity option configured"
                );
                return Err(format!(
                    "{} is not a valid InMemoryBuffer capacity_option",
                    other
                ));
            }
        };

        let durability = match &buffer_config.durability {
            DurabilityConfig::InMemory => {
                tracing::warn!(
                    "InMemoryBuffer durability set to Durability::InMemory, no persistence support. \
                    We do not recomment running in this configuration in production deployments"
                );
                Durability::InMemory
            }
            DurabilityConfig::SQLite(path) => {
                tracing::info!(
                    sqlite_db_path = ?path,
                    "InMemoryBuffer durability set to Durability::SQLite, persistence support is active"
                );

                let manager = SqliteConnectionManager::file(path);
                let pool = Pool::new(manager).map_err(|e| {
                    tracing::error!(
                        error = %e,
                        "Failed to create SQLite connection pool"
                    );
                    format!("Failed to create SQLite connection pool: {}", e)
                })?;

                let conn = pool.get().map_err(|e| {
                    tracing::error!(
                        error = %e,
                        "Failed to get pooled SQLite connection"
                    );
                    format!("Failed to get pooled SQLite connection: {}", e)
                })?;
                conn.execute(
                    "CREATE TABLE IF NOT EXISTS normalized_logs (
                        id              INTEGER PRIMARY KEY AUTOINCREMENT,
                        timestamp       TEXT NOT NULL,
                        level           TEXT,
                        message         TEXT NOT NULL,
                        metadata        TEXT,
                        raw_line        TEXT NOT NULL
                    )",
                    (),
                )
                .map_err(|e| {
                    tracing::error!(
                        error = %e,
                        sqlite_db_path = %path,
                        "Failed to create normalized_logs table in SQLite database"
                    );
                    format!(
                        "Failed to create normalized_logs table in SQLite database: {}",
                        e
                    )
                })?;

                Durability::SQLite(Arc::new(pool))
            }
        };

        Ok(Self {
            queue,
            buffer_capacity: buffer_config.buffer_capacity,
            batch_size: buffer_config.batch_size,
            batch_timeout_ms: buffer_config.batch_timeout_ms,
            last_flush_at: Instant::now(),
            durability,
            overflow_policy: buffer_config.overflow_policy.clone(),
            drain_policy: buffer_config.drain_policy.clone(),
            flush_policy: buffer_config.flush_policy.clone(),
            notify: Arc::new(Notify::new()),
        })
    }

    /// Gracefully shuts down the InMemoryBuffer.
    ///
    /// `shutdown()` ensures:
    /// - All remaining logs in-memory are flushed to persistence.
    /// - Any blocked tasks waiting on `notify` are released.
    /// - The underlying durability layer (SQLite or InMemory) is gracefully closed.
    /// - No further writes should be attempted after shutdown.
    #[instrument(
        name = "ves_inmemory_buffer_shutdown_signal",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    pub async fn shutdown(&mut self) -> Result<()> {
        tracing::warn!("Shutdown signal received, InMemoryBuffer shutdown initiated");

        // Wake all waiting tasks, ensuring no task remains blocked waiting
        // for capacity during shutdown
        self.notify.notify_waiters();

        // Flush all remaining logs
        if !self.queue.is_empty() {
            tracing::info!(
                logs_in_inmemory_buffer = %self.queue.len(),
                "Flushing remaining logs in InMemoryBuffer befor shutdown"
            );

            if let Err(err) = self.flush_remaining_logs().await {
                tracing::error!(
                    error = %err,
                    "Error flushing remaining logs from InMemoryBuffer"
                );
            } else {
                tracing::info!(
                    logs_in_inmemory_buffer = %self.queue.len(),
                    "Successfully flushed remaining logs from InMemoryBuffer"
                );
            }
        } else {
            tracing::info!("No remaining logs to flush from InMemoryBuffer");
        }

        // Release durability layer resources
        match &mut self.durability {
            Durability::SQLite(pool) => {
                tracing::info!("Releasing SQLite connection pool");
                // Dropping Arc<Pool> reference allows connections to close naturally.
                // No explicit close method exists for r2d2 pools.
                let _ = Arc::get_mut(pool);
            }
            Durability::InMemory => {
                tracing::info!(
                    "Currently using in-memory durability, no persistent SQLite connection pool to close"
                );
            }
        }

        tracing::info!("InMemoryBuffer successfully shutdown gracefully");

        Ok(())
    }

    /// Check InMemoryBuffer's capacity. This is required to determine
    /// actions in push(), drain(), flush() methods.
    pub fn check_buffer_capacity(&self) -> usize {
        let used_capacity = self.queue.len();

        if self.buffer_capacity == 0 {
            usize::MAX
        } else {
            self.buffer_capacity as usize - used_capacity
        }
    }

    /// Asynchronously push a `NormlizedLog` to an `InMemoryBuffer`.
    ///
    /// Logs in the buffer are persisted to SQLite after each `push_back`
    /// to the buffer. If persistence configuration is set to `SQLite`
    #[instrument(
        name = "ves_inmemory_buffer_push",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    pub async fn push(&mut self, log: NormalizedLog) -> Result<()> {
        // TODO: increment_counter!("normalizedlogs.buffered");

        if self.buffer_capacity > 0 && self.queue.len() >= self.buffer_capacity as usize {
            tracing::warn!(
                inmemory_buffer_capacity = %self.buffer_capacity,
                inmemory_buffer_queue_len = %self.queue.len(),
                "InMemoryBuffer overflowing, dropping logs or handling overflow"
            );
            self.handle_overflow();
        }

        match &mut self.durability {
            /*
             * In-memory mode
             */
            Durability::InMemory => {
                self.queue.push_back(log);
            }

            /*
             * SQLite mode
             */
            Durability::SQLite(_pool) => {
                self.queue.push_back(log.clone());

                // TODO: increment_counter!("normalizedlogs.persisted_attempt");

                if let Err(e) = self.flush(log.clone()).await {
                    // TODO: increment_counter!("normalizedlogs.persisted_fail");

                    tracing::error!(
                        error = %e,
                        "Failed to persist NormalizedLog to SQLite db"
                    );
                    return Err(e);
                }

                // TODO: increment_counter!("normalizedlogs.persisted_success");
            }
        }

        Ok(())
    }

    /// Flush a `NormalizedLog` batch to an SQLite database(if persistence is enabled), while
    /// clearing the InMemoryBuffer of the flushed logs, to prevent unnecessary `NormalizedLog`
    /// duplication.
    ///
    /// Returns flushed `NormalizedLog` batch after flush is triggered.
    #[instrument(
        name = "ves_inmemory_buffer_flush",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    pub async fn flush(&mut self, log: NormalizedLog) -> Result<Option<InMemoryBuffer>> {
        // Variables to store user configured flush triggers
        let flush_by_batch_size = self.queue.len() >= self.batch_size;
        let flush_by_batch_timeout_ms = Instant::now().duration_since(self.last_flush_at)
            > Duration::from_millis(self.batch_timeout_ms);

        // Determine if flushing based on policy
        let should_flush = match self.flush_policy.as_str() {
            "batch_size" => flush_by_batch_size,
            "batch_timeout" => flush_by_batch_timeout_ms,
            "hybrid_size_timeout" => flush_by_batch_size || flush_by_batch_timeout_ms,
            other => {
                tracing::error!(
                    invalid_flush_policy = ?other,
                    "Invalid flush_policy configured, skipping InMemoryBuffer flush"
                );
                false
            }
        };

        if !should_flush {
            tracing::warn!(
                flush_policy = %self.flush_policy,
                queue_len = %self.queue.len(),
                "InMemoryBuffer flush not triggered: NormalizedLogs remain in InMemoryBuffer"
            );
            return Ok(None); // Nothing to flush
        }

        // Determine how many logs to flush
        let flush_count =
            if self.flush_policy == "batch_size" || self.flush_policy == "hybrid_size_timeout" {
                tracing::info!(
                    policy = %self.flush_policy,
                    batch_size = %self.batch_size,
                    queue_len = %self.queue.len(),
                    flush_count = %self.batch_size.min(self.queue.len()),
                    "Flushing InMemoryBuffer NormalizedLogs based on batch_size policy"
                );
                self.batch_size.min(self.queue.len())
            } else {
                tracing::info!(
                    policy = %self.flush_policy,
                    queue_len = %self.queue.len(),
                    flush_count = %self.queue.len(),
                    "Flushing all NormalizedLogs in InMemoryBuffer"
                );
                self.queue.len()
            };

        // Collect the logs being flushed in an InMemoryBuffer
        let drained_logs: Vec<NormalizedLog> = self.queue.drain(..flush_count).collect();
        tracing::debug!(
            drained_logs_count = %drained_logs.len(),
            "Drained NormalizedLogs from InMemoryBuffer for flush"
        );

        // Wrap collected logs from log_batch in an InMemoryBuffer
        let buffer = InMemoryBuffer {
            queue: VecDeque::from(drained_logs),
            buffer_capacity: self.buffer_capacity,
            batch_size: self.batch_size,
            batch_timeout_ms: self.batch_timeout_ms,
            last_flush_at: self.last_flush_at,
            durability: self.durability.clone(),
            overflow_policy: self.overflow_policy.clone(),
            drain_policy: self.drain_policy.clone(),
            flush_policy: self.flush_policy.clone(),
            notify: self.notify.clone(),
        };

        // Perform actual flush based on configured durability
        match &mut self.durability {
            Durability::SQLite(pool) => {
                tracing::debug!(
                    inmemory_buffer_flush_count = %buffer.queue.len(),
                    "Persisting NormalizedLogs to SQLite db"
                );

                let mut conn = pool
                    .get()
                    .expect("Failed to get DB connection from connection pool");
                let tx = conn.transaction()?; // begin transaction

                for log in buffer.queue.iter() {
                    tx.execute(
                        "INSERT INTO normalized_logs (timestamp, level, message, metadata, raw_line)
                        VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            log.timestamp.to_rfc3339(),
                            log.level,
                            log.message,
                            log.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap()),
                            log.raw_line
                        ],
                    )?;
                }
                tx.commit()?; // commit transaction once
                self.last_flush_at = Instant::now();

                tracing::info!(
                    persisted_count = %buffer.queue.len(),
                    "Successfully persisted NormalizedLogs to SQLite"
                );

                Ok(Some(buffer))
            }
            Durability::InMemory => {
                self.last_flush_at = Instant::now();
                tracing::info!(
                    inmemory_buffer_flush_count = %buffer.queue.len(),
                    "Durability is Durability::InMemory: NormalizedLogs flushed from InMemoryBuffer but not persisted to SQLite"
                );
                Ok(Some(buffer))
            }
        }
    }

    #[instrument(
        name = "ves_inmemory_buffer_flush_remaining_logs",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    pub async fn flush_remaining_logs(&mut self) -> Result<()> {
        if self.queue.is_empty() {
            tracing::debug!(
                remaining_logs = 0,
                "No remaining NormalizedLogs in InMemoryBuffer to flush"
            );
            return Ok(());
        }

        tracing::info!(
            remaining_logs = %self.queue.len(),
            "Flushing remaining NormalizedLogs from InMemoryBuffer"
        );
        let drained_logs: Vec<NormalizedLog> = self.queue.drain(..).collect();
        tracing::debug!(
            drained_logs_count = %drained_logs.len(),
            "Drained NormalizedLogs from InMemoryBuffer for final flush"
        );

        match &mut self.durability {
            Durability::SQLite(pool) => {
                tracing::debug!(
                    inmemory_buffer_flush_count = %drained_logs.len(),
                    "Persisting remaining logs for SQLite"
                );

                let mut conn = pool
                    .get()
                    .expect("Failed to get DB connection from connection pool");
                let tx = conn.transaction()?;

                for log in drained_logs.iter() {
                    tx.execute(
                        "INSERT INTO normalized_logs (timestamp, level, message, metadata, raw_line)
                        VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            log.timestamp.to_rfc3339(),
                            log.level,
                            log.message,
                            log.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap()),
                            log.raw_line
                        ],
                    )?;
                }
                tx.commit()?;

                tracing::info!(
                    persisted_count = %drained_logs.len(),
                    "Successfully persisted remaining NormalizedLogs to SQLite"
                );
            }
            Durability::InMemory => {
                tracing::info!(
                    inmemory_buffer_flush_count = %drained_logs.len(),
                    "Durability is Durability::InMemory: remaining NormalizedLogs drained from InMemoryBuffer but not persisted to SQLite"
                );
            }
        }

        self.last_flush_at = Instant::now();
        Ok(())
    }

    /// Drain a `NormalizedLog` batch removing logs from in-memory queue, freeing up
    /// space and ensuring an `InMemoryBuffer` remains performant at all times.
    ///
    /// `InMemoryBuffer` draining is dependant on configured **drain_policy** and
    /// **flush_policy**, drain policies are intended to work with certain flush
    /// policies. Users should configure flush and drain policies while considering
    /// how they want both to work.
    ///
    /// **Use Cases**:
    /// - `NormalizedLog` batch has been flushed to SQLite persistence **flush()**.
    #[instrument(
        name = "ves_inmemory_buffer_drain",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    pub async fn drain(&mut self) -> Result<()> {
        match self.drain_policy.as_str() {
            "drain_all" => {
                let drained_count = self.queue.len();
                self.queue.drain(..drained_count);
                tracing::info!(
                    policy = %self.drain_policy,
                    drained_count = %drained_count,
                    "Drained all NormalizedLogs from InMemoryBuffer"
                );
                Ok(())
            }

            "drain_batch_size" => {
                let drained_count = if self.queue.len() >= self.batch_size {
                    self.queue.drain(..self.batch_size).count()
                } else {
                    0
                };
                tracing::info!(
                    policy = %self.drain_policy,
                    batch_size = %self.batch_size,
                    drained_count = %drained_count,
                    "Drained NormalizedLogs based on batch_size from InMemoryBuffer"
                );
                Ok(())
            }

            "drain_batch_timeout" => {
                let elapsed_ms = Instant::now()
                    .duration_since(self.last_flush_at)
                    .as_millis();
                let drained_count = if elapsed_ms > self.batch_timeout_ms as u128 {
                    let count = self.queue.len();
                    self.queue.drain(..count);
                    count
                } else {
                    0
                };
                tracing::info!(
                    policy = %self.drain_policy,
                    elapsed_ms = %elapsed_ms,
                    drained_count = %drained_count,
                    "Drained NormalizedLogs based on batch_timeout from InMemoryBuffer"
                );
                Ok(())
            }

            other => {
                tracing::error!(
                    invalid_drain_policy = ?other,
                    "Invalid drain_policy configured. NormalizedLogs not drained from InMemoryBuffer"
                );
                Ok(())
            }
        }
    }

    /// Handle `InMemoryBuffer` overflowing when capacity is approaching
    /// or exceeding user configured buffer_capacity.
    ///
    /// This ensures buffer_capacity is not exceeded while pushing `NormalizedLog`
    /// to `InMemoryBuffer`. An `InMemoryBuffer` overflowing is handled prior to pushing
    /// `NormalizedLog` to it.
    #[instrument(
        name = "ves_inmemory_buffer_overflow",
        target = "buffer_batcher::log_buffer_batcher::InMemoryBuffer",
        skip_all,
        level = "trace"
    )]
    async fn handle_overflow(&mut self) -> Result<bool> {
        match self.overflow_policy.as_str() {
            "drop_newest" => {
                tracing::warn!(
                    policy = %self.overflow_policy,
                    buffer_capacity = %self.buffer_capacity,
                    queue_len = %self.queue.len(),
                    "InMemoryBuffer full: dropping newest incoming NormalizedLog"
                );
                Ok(false)
            }
            "drop_oldest" => {
                let dropped = self.queue.pop_front();
                tracing::warn!(
                    policy = %self.overflow_policy,
                    dropped_log_present = %dropped.is_some(),
                    queue_len_after = %self.queue.len(),
                    "InMemoryBuffer full: dropping oldest NormalizedLog"
                );
                Ok(true)
            }
            "block_with_backpressure" => {
                tracing::warn!(
                    policy = %self.overflow_policy,
                    buffer_capacity = %self.buffer_capacity,
                    queue_len = %self.queue.len(),
                    "InMemoryBuffer full: applying backpressure, waiting until queue has space"
                );
                while self.queue.len() >= self.buffer_capacity as usize {
                    self.notify.notified().await;
                }
                tracing::info!(
                    queue_len_after = %self.queue.len(),
                    "InMemoryBuffer backpressure released, space available in queue"
                );
                Ok(false)
            }
            "grow_capacity" => {
                self.queue.reserve(1);
                tracing::warn!(
                    policy = %self.overflow_policy,
                    new_capacity = %self.queue.capacity(),
                    "InMemoryBuffer full: deque capacity exceeded, capacity extended dynamically"
                );
                Ok(true)
            }
            other => {
                tracing::error!(
                    invalid_overflow_policy = ?other,
                    "Invalid overflow_policy configured, InMemoryBuffer overflow is unhandled!"
                );
                Ok(false)
            }
        }
    }
}

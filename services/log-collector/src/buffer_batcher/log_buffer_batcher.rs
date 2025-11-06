use crate::helpers::load_config::{BufferConfig, DurabilityConfig};
use crate::parser::parser::NormalizedLog;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Result, params};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::Instant;

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
    pub async fn new(buffer_config: &BufferConfig) -> Self {
        let queue = match buffer_config.capacity_option.as_str() {
            "bounded" => {
                println!(
                    "Creating bounded buffer with capacity {}",
                    buffer_config.buffer_capacity
                );
                VecDeque::with_capacity(buffer_config.buffer_capacity as usize)
            }
            "unbounded" => {
                println!("Creating unbounded buffer");
                VecDeque::new()
            }
            other => {
                panic!("{} is not a valid buffer capacity_option", other);
            }
        };

        let durability = match &buffer_config.durability {
            DurabilityConfig::InMemory => {
                println!(
                    "Using in-memory buffer (no persistence support). \
                    We do not recommend running in this configuration in production deployments."
                );
                Durability::InMemory
            }
            DurabilityConfig::SQLite(path) => {
                println!("Creating persistent buffer with SQLite support at {}", path);

                let manager = SqliteConnectionManager::file(path);
                let pool = Pool::new(manager)
                    .unwrap_or_else(|e| panic!("Failed to create SQLite pool: {}", e));

                let conn = pool.get().expect("Failed to get pooled SQLite connection");
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
                .expect("Failed to create normalized_logs table in SQLite");

                Durability::SQLite(Arc::new(pool))
            }
        };

        InMemoryBuffer {
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
        }
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
    /// to the buffer.
    pub async fn push(&mut self, log: NormalizedLog) -> Result<()> {
        if self.buffer_capacity > 0 && self.queue.len() >= self.buffer_capacity as usize {
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

                self.flush(log.clone()).await?;
            }
        }

        Ok(())
    }

    /// Flush a `NormalizedLog` batch to an SQLite database(if persistence is enabled), while
    /// clearing the InMemoryBuffer of the flushed logs, to prevent unnecessary `NormalizedLog`
    /// duplication.
    ///
    /// Returns flushed `NormalizedLog` batch after flush is triggered.
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
                eprintln!(
                    "No correct flush_policy configured. '{}' is not a flush_policy option",
                    other
                );
                false
            }
        };

        if !should_flush {
            return Ok(None); // Nothing to flush
        }

        // Determine how many logs to flush
        let flush_count =
            if self.flush_policy == "batch_size" || self.flush_policy == "hybrid_size_timeout" {
                self.batch_size.min(self.queue.len())
            } else {
                self.queue.len()
            };

        // Collect the logs being flushed in an InMemoryBuffer
        let drained_logs: Vec<NormalizedLog> = self.queue.drain(..flush_count).collect();

        // Wrap collected logs from log_batch in InMemoryBuffer
        let buffer = InMemoryBuffer::from(drained_logs);

        // Perform actual flush based on configured durability
        match &mut self.durability {
            Durability::SQLite(pool) => {
                let conn = pool.get().expect("Failed to get connection from pool");
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
                Ok(Some(buffer))
            }
            Durability::InMemory => {
                self.last_flush_at = Instant::now();
                eprintln!(
                    "Durability set to `InMemory`: returning flushed logs without persistence to SQLite."
                );
                Ok(Some(buffer))
            }
        }
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
    pub async fn drain(&mut self) -> Result<()> {
        match self.drain_policy.as_str() {
            "drain_all" => {
                self.queue.drain(..self.queue.len());
                Ok(())
            }
            "drain_batch_size" => {
                if self.queue.len() >= self.batch_size {
                    self.queue.drain(..self.batch_size);
                }
                Ok(())
            }
            "drain_batch_timeout" => {
                if Instant::now().duration_since(self.last_flush_at)
                    > Duration::from_millis(self.batch_timeout_ms)
                {
                    self.queue.drain(..self.queue.len());
                }
                Ok(())
            }
            other => {
                eprintln!(
                    "No correct drain_policy configured. {} is not a drain_policy option",
                    other
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
    async fn handle_overflow(&mut self) -> Result<bool> {
        match self.overflow_policy.as_str() {
            "drop_newest" => {
                eprintln!("Buffer full: dropping newest incoming log");
                Ok(false)
            }
            "drop_oldest" => {
                self.queue.pop_front();
                Ok(true)
            }
            "block_with_backpressure" => {
                while self.queue.len() >= self.buffer_capacity as usize {
                    self.notify.notified().await;
                }
                Ok(false)
            }
            "grow_capacity" => {
                self.queue.reserve(1);
                eprintln!(
                    "buffer_capacity exceeded, capacity extended without memory re-allocation"
                );
                Ok(true)
            }
            other => {
                eprintln!(
                    "No overflow_policy configured, {} is not a configuration option",
                    other
                );
                Ok(false)
            }
        }
    }
}

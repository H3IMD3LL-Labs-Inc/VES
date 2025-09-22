// Components
// 1. Buffer
// Collects parsed/enriched logs into in-memory Buffer. If durability is set to sqlite,
// also appends logs into a WAL table for persistence. Basically, acts like a queue ->
// fast in-memory, optional safety net on disk(production).
//
// 2. Batcher
// Watches Buffers, has 2 flush conditions; Size trigger(number of logs), Time trigger(time since last flush).
// When triggered; collects a Batch from Buffer (and deletes from WAL if enabled), compresses the batch, returns
// a compressed batch for use by the shipper.
//
// All behavior is defined by log-collector.toml, with the following basic control flow;
// 1. Log arrives -> Push to in-memory, if durable set also append to WAL
// 2. Batcher checks conditions -> if enough logs OR time elapsed: create batch
// 3. Batch creation -> Drain logs from in-memory, if durable set delete correspondence from WAL
// 4. Batch processing -> Compress with gzip, return compressed batch(to be sent to embedding service via Shipper)
// 5. Failure handling -> in-memory mode: logs are already gone if process dies/crashes, durable mode: logs still
// WAL -> retry on restart.
//

use crate::parser::parser::NormalizedLog;
use rusqlite::{Connection, Result, params};
use serde::Deserialize;
use tokio::time::Instant;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::task;
use tokio::sync::Notify;

/// Configuration
#[derive(Debug, Deserialize)]
struct Config {
    pub buffer: BufferConfig,
}

/// Buffer configuration
#[derive(Debug, Deserialize)]
struct BufferConfig {
    capacity_option: String,
    buffer_capacity: u64,
    batch_size: usize,
    batch_timeout_ms: u64,
    durability: DurabilityConfig,
    overflow_policy: String,
    drain_policy: String,
    flush_policy: String,
}

/// In-Memory Buffer (runtime structure)
#[derive(Debug)]
struct InMemoryBuffer {
    queue: VecDeque<NormalizedLog>,
    buffer_capacity: u64,
    batch_size: usize,
    batch_timeout_ms: u64,
    last_flush_at: Instant,
    durability: Durability,
    overflow_policy: String,
    drain_policy: String,
    flush_policy: String,
}

/// Configuration-only durability
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum DurabilityConfig {
    InMemory,
    SQLite(String),
}

/// Runtime durability (contains real durability resources used by InMemoryBuffer)
#[derive(Debug)]
pub enum Durability {
    InMemory,
    SQLite(Connection),
}

impl InMemoryBuffer {
    pub async fn load_config(path: &str) -> BufferConfig {
        let buffer_config = std::fs::read_to_string(path).expect("Failed to read config file");

        toml::from_str(&buffer_config).expect("Failed to parse config file")
    }

    /// Create a new `InMemoryBuffer` to store `NormalizedLog`
    /// produced by the `parser`.
    ///
    /// The new buffer is configured at runtime based on user
    /// configuration set in `log-collector.toml` file.
    ///
    /// **Recommended:** For use in production environments, ensure
    /// buffer durability is set to `persistent => { sqlite = "parsed_log_buffer.db" }`
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
                let conn = Connection::open(path)
                    .unwrap_or_else(|e| panic!("Failed to open SQLite DB {}: {}", path, e));

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

                Durability::SQLite(conn)
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
        }
    }

    /// Check `InMemoryBuffer` capacity, for health/performance checks.
    /// Currently, this has no implemented use case.
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
            Durability::SQLite(conn) => {
                self.queue.push_back(log.clone());

                self.flush(log.clone());
            }
        }

        Ok(())
    }

    /// Flush a `NormalizedLog` batch to SQLite if persistence is enabled,
    /// while clearing the InMemoryBuffer of the flushed logs, to prevent
    /// unnecessary duplication overhead.
    ///
    /// [Coming Soon]: Flush logs to SQLite persistence without clearing
    /// InMemoryBuffer, to enable fast lookup by the `Shipper`, without requiring calls
    /// SQLite persistence.
    pub async fn flush(&mut self, log: NormalizedLog) -> Result<()> {
        match self.flush_policy.as_str() {
            "batch_size" => {
                if self.queue.len() >= self.batch_size {
                    match &mut self.durability {
                        Durability::SQLite(conn) => {
                            let batch_logs: Vec<NormalizedLog> = self.queue
                                .iter()
                                .take(self.batch_size)
                                .cloned()
                                .collect();

                            for log in batch_logs {
                                conn.execute(
                                    "INSERT INTO normalized_logs (timestamp, level, message, metadata, raw_line) VALUES (?1, ?2, ?3, ?4, ?5)",
                                    params![
                                        log.timestamp.to_rfc3339(),
                                        log.level,
                                        log.message,
                                        log.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap()),
                                        log.raw_line
                                    ],
                                )?;
                            }
                            self.last_flush_at = Instant::now();

                            self.drain();
                        }
                        Durability::InMemory => {
                            eprintln!("InMemoryBuffer durability configured to `in-memory` logs are currently not flushed to SQLite persistent storage");
                        }
                    }
                }

                Ok(())
            }
            "batch_timeout" => {
                if Instant::now().duration_since(self.last_flush_at) > Duration::from_millis(self.batch_timeout_ms) {
                    match &mut self.durability {
                        Durability::SQLite(conn) => {

                            let batch_logs: Vec<NormalizedLog> = self.queue
                                .iter()
                                .take(self.queue.len())
                                .cloned()
                                .collect();

                            for log in batch_logs {
                                conn.execute(
                                    "INSERT INTO normalized_logs (timestamp, level, message, metadata, raw_line) VALUES (?1, ?2, ?3, ?4, ?5)",
                                    params![
                                        log.timestamp.to_rfc3339(),
                                        log.level,
                                        log.message,
                                        log.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap()),
                                        log.raw_line,
                                    ],
                                )?;
                            }
                            self.last_flush_at = Instant::now();

                            self.drain();
                        }
                        Durability::InMemory => {
                            eprintln!("InMemoryBuffer durability configured to `in-memory` logs are currently not flushed to SQLite persistent storage");
                        }
                    }
                }

                Ok(())
            }
            "hybrid_size_timeout" => {
                if self.queue.len() >= self.batch_size {
                    match &mut self.durability {
                        Durability::SQLite(conn) => {
                            let batch_logs: Vec<NormalizedLog> = self.queue
                                .iter()
                                .take(self.batch_size)
                                .cloned()
                                .collect();

                            for log in batch_logs {
                                conn.execute(
                                    "INSERT INTO normalized_logs (timestamp, level, message, metadata, raw_line) VALUES (?1, ?2, ?3, ?4, ?5)",
                                    params![
                                        log.timestamp.to_rfc3339(),
                                        log.level,
                                        log.message,
                                        log.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap()),
                                        log.raw_line
                                    ],
                                )?;
                            }
                            self.last_flush_at = Instant::now();

                            self.drain();
                        }
                        Durability::InMemory => {
                            eprintln!("InMemoryBuffer durability configured to `in-memory`, logs are currently not being persisted to SQLite");
                        }
                    }
                }
                if Instant::now().duration_since(self.last_flush_at) > Duration::from_millis(self.batch_timeout_ms) {
                    match &mut self.durability {
                        Durability::SQLite(conn) => {
                            let batch_logs: Vec<NormalizedLog> = self.queue
                                .iter()
                                .take(self.queue.len())
                                .cloned()
                                .collect();

                            for log in batch_logs {
                                conn.execute(
                                    "INSERT INTO normalized_logs (timestamp, level, message, metadata, raw_line) VALUES (?1, ?2, ?3, ?4, ?5)",
                                    params![
                                        log.timestamp.to_rfc3339(),
                                        log.level,
                                        log.message,
                                        log.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap()),
                                        log.raw_line,
                                    ],
                                )?;
                            }
                            self.last_flush_at = Instant::now();

                            self.drain();
                        }
                        Durability::InMemory => {
                            eprintln!("InMemoryBuffer durability configured to `in-memory`, logs are currently not being persisted to SQLite")
                        }
                    }
                }

                Ok(())
            }
            other => {
                eprintln!("No correct flush_policy configured. {} is not a flush_policy option", other);
                Ok(())
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
                if Instant::now().duration_since(self.last_flush_at) > Duration::from_millis(self.batch_timeout_ms) {
                    self.queue.drain(..self.queue.len());
                }
                Ok(())
            }
            other => {
                eprintln!("No correct drain_policy configured. {} is not a drain_policy option", other);
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
                // When buffer is full, reject incoming log.
                // Preserve all older logs and silently discard/ignore
                // newest one
                eprintln!("Buffer full: dropping newest incoming log");
                Ok(false)
            }
            "drop_oldest" => {
                // When buffer is full, evict the oldest log to
                // make room. Always accept the newest log, oldest
                // history is discarded first
                self.queue.pop_front();
                Ok(true)
            }
            "block_with_backpressure" => {
                // When buffer is full, producer must wait until
                // there's space. No log is dropped, but log
                // ingestion slows down.
                while self.queue.len() >= self.buffer_capacity as usize {
                    self.notify.notified().await;
                }
                Ok(false)
            }
            "grow_capacity" => {
                // When buffer is full, increase its capacity dynamically
                // No log is ever dropped or blocked, buffer grows as needed.
                // Prevent producers from oupacing consumers, to prevent OOM
                // (out-of-memory).
                self.queue.reserve(1);
                eprintln!("buffer_capacity exceeded, capacity extended without memory re-allocation")
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

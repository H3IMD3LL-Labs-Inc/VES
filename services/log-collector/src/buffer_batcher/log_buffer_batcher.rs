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
use std::collections::VecDeque;
use std::sync::Arc;
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
}

/// In-Memory Buffer (runtime structure)
#[derive(Debug)]
struct InMemoryBuffer {
    queue: VecDeque<NormalizedLog>,
    buffer_capacity: u64,
    batch_size: usize,
    batch_timeout_ms: u64,
    durability: Durability,
    overflow_policy: String,
    drain_policy: String,
    notify: Arc<Notify>,
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

/// Methods implementing Buffer
impl InMemoryBuffer {
    pub async fn load_config(path: &str) -> BufferConfig {
        let buffer_config = std::fs::read_to_string(path).expect("Failed to read config file");

        toml::from_str(&buffer_config).expect("Failed to parse config file")
    }

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
            durability,
            overflow_policy: buffer_config.overflow_policy.clone(),
            drain_policy: buffer_config.drain_policy.clone(),
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

    /// Push NormalizedLogs into an InMemoryBuffer
    // If buffer is bounded and full -> enforce overflow_policy

    // IMPLEMENTATION
    // Goal: Combine fast in-memory writes + durable SQLite persistence while minimizing data loss.
    // 1. In-memory queue: every log is pushed here immediately -> very fast
    // 2. Async batch flush: periodically take logs from memory -> insert into SQLite -> improve throughput.
    // 3. Durability safety: prevent data loss if process crashes before batch flush;
    // - WAL
    // - Sync flush on shutdown/critical events
    // - Configurable durability mode (WAL and Sync flush on shutdown/critical events)
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
        }

        Ok(())
    }

    /// Asynchronously flush batch to SQLite if persistence is enabled,
    /// while clearing the InMemoryBuffer (handle with care).
    ///
    /// This does not clear logs from InMemoryBuffer after they are persisted
    /// to SQLite
    ///
    /// This maintains efficiency, and reduces transaction overhead, improving throughput.
    pub async fn flush() -> Result<()> {}

    /// Asynchronously drain batch, removing logs from in-memory queue after
    /// successful and confirmed consumption.
    ///
    /// Use the set `drain_policy` to determine how to drain the batch
    pub async fn drain() -> Result<()> {}

    /// Handle InMemoryBuffer overflow using the overflow_policy
    /// set in log_collector.toml by user.
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

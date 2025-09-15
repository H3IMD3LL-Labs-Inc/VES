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
use tokio::fs;

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
    pub async fn load_config(path: &str) -> Config {
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

    pub async fn add_normalized_log(&mut self, log: NormalizedLog) {
        // Check if log being added is NormalizedLog
        // Check durability, capacity, overflow policy and drain policy configuration
        // Manage async code, should I use RwLocks-Mutexes-Atomic operations or simple tokio::sync::mspc?
    }
}

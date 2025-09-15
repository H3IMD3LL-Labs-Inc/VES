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
    buffer: InMemoryBuffer,
    buffer_capacity: u64,
    batch_size: usize,
    batch_timeout_ms: u64,
    durability: String,
    sqlite_path: Option<String>,
    overflow_policy: String,
    drain_policy: String,
}

/// In-Memory Buffer
#[derive(Debug, Deserialize)]
struct InMemoryBuffer {
    queue: VecDeque<NormalizedLog>,
    buffer_capacity: u64,
    batch_size: usize,
    batch_timeout_ms: u64,
    durability: String,
    sqlite_path: Option<String>,
    overflow_policy: String,
    drain_policy: String,
}

/// Methods implementing Buffer
impl InMemoryBuffer {
    pub async fn load_config(path: &str) -> Config {
        let buffer_config = std::fs::read_to_string(path).expect("Failed to read config file");

        toml::from_str(&buffer_config).expect("Failed to parse config file")
    }

    pub async fn new(buffer_config: &BufferConfig) -> Self {
        let queue = VecDeque::new();

        if buffer_config.durability == "sqlite" {
            if let Some(path) = &buffer_config.sqlite_path {
                println!("Opening SQLite supported buffer at {}", path);

                // TODO: Create SQLite instance for the new buffer persistence
            } else {
                panic!("Durability is set to 'sqlite' but no sqlite_path provided!");
            }
        } else {
            println!("Using in-memory buffer (no persistence)");
        }

        // Check capacity(bounded or unbounded), overflow policy, drain policy

        InMemoryBuffer {
            queue,
            buffer_capacity: buffer_config.buffer_capacity,
            batch_size: buffer_config.batch_size,
            batch_timeout_ms: buffer_config.batch_timeout_ms,
            durability: buffer_config.durability.clone(),
            sqlite_path: buffer_config.sqlite_path.clone(),
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

//

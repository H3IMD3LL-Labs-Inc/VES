use rand::Rng;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tonic::transport::{Channel, Endpoint};

use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::proto::common::NormalizedLog;
use crate::proto::embedder::{EmbedResponse, embedder_client::EmbedderClient};

/// Configuration
/// - Wraps `ShipperConfig`, making it possible to embed into `log_collector.toml`.
#[derive(Debug, Deserialize)]
struct Config {
    pub shipper: ShipperConfig,
}

/// Shipper Configuration
/// - Defines tunable configuration for Shipper at runtime
#[derive(Debug, Deserialize)]
struct ShipperConfig {
    embedder_target_addr: String,
    connection_timeout_ms: u64,
    max_reconnect_attempts: Option<u64>,
    initial_retry_delay_ms: u64,
    max_retry_delay_ms: u64,
    backoff_factor: f64,
    retry_jitter: f64,
    send_timeout_ms: u64,
    response_timeout_ms: u64,
    metrics_enabled: bool,
    log_level: String,
}

/// Shipper (runtime struct)
/// - Holds sending side of the channel. `run_worker` has the other side,
/// spawned as a background task. Ensuring decoupling, shipper runs independently of
/// other log collector functionality.
#[derive(Debug)]
struct Shipper {
    sender: mpsc::Sender<InMemoryBuffer>,
}

/// Shipper error handling
/// - Cleary defines domain errors; `QueueFull`, `ConnectionFailed`. Allowing easier upstream
/// error propagation.
#[derive(Debug, thiserror::Error)]
pub enum ShipperError {
    #[error("Queue is full")]
    QueueFull,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("stream error: {0}")]
    StreamError(String),
}

impl Shipper {
    /// Read `log_collector.toml` Shipper configuration and deserialize into `ShipperConfig`.
    /// Allows tweaking Shipper behaviour at runtime without code changes.
    pub async fn load_config(path: &str) -> ShipperConfig {
        let shipper_config = std::fs::read_to_string(path).expect("Failed to read config file");

        toml::from_str(&shipper_config).expect("Failed to parse config file")
    }

    /// Create a new Shipper instance with a background worker
    /// - Creates a small channel, spawns the background worker(`run_worker`) with receiving end,
    /// returns `Shipper` with the sending side. Ensuring each Shipper runs its own async loop in the
    /// background.
    pub async fn new(config: ShipperConfig) -> Self {
        // Small channel to decouple producer and worker
        let (tx, rx) = mpsc::channel(1); // maybe 5-10, for some slack
        tokio::spawn(run_worker(config.clone(), rx));
        Self { sender: tx }
    }

    /// Public API: send flushed batch from buffer-batcher
    /// - Actual API exposed to the rest of the collector pipeline. Takes an `InMemoryBuffer`, pushes into
    /// channel -> non-blocking hand-off to worker. If full, returns `QueueFull` error.
    /// - This is the public-accessible surface for the Shipper.
    pub async fn send(&self, batch: InMemoryBuffer) -> Result<(), ShipperError> {
        self.sender
            .send(batch)
            .await
            .map_err(|_| ShipperError::QueueFull)
    }
}

/// Background worker that owns the gRPC connection
///
/// Infinite loop:
/// - Try to connect with retries(`connect_with_retry`)
/// - If connected:
///     - Open bi-directional stream (`embed_log()` RPC).
///     - Split into:
///         - `send_loop`: pulls batches from channel -> forwards logs downstream.
///         - `recv_loop`: listens for `EmbedResponse` from embedder.
///     - Uses `tokio::select!` to multiplex send/recv.
/// - On error -> reconnect logic triggers
async fn run_worker(config: ShipperConfig, mut rx: mpsc::Receiver<InMemoryBuffer>) {
    loop {
        match connect_with_retry(&config).await {
            Ok(mut client) => {
                // Open bi-directional streaming RPC
                let stream = client.embed_log().await;
                match stream {
                    Ok(mut stream) => {
                        // Split stream into sender and receiver halves
                        let (mut req_tx, mut resp_rx) = stream.into_parts();

                        loop {
                            tokio::select! {
                                maybe_batch = rx.recv() => {
                                    match maybe_batch {
                                        Some(batch) => {
                                            for log in batch.queue {
                                                if let Err(e) = req_tx.send(log).await {
                                                    eprintln!("Failed to send log: {:?}", e);
                                                    break; // trigger reconnect attempt(s)
                                                }
                                            }
                                        }
                                        None => break, // channel closed
                                    }
                                }
                                resp = resp_rx.message() => {
                                    match resp {
                                        Ok(Some(embed_resp)) => handle_embed_response(embed_resp),
                                        Ok(None) => {
                                            eprintln!("Embedder closed response stream");
                                            break;
                                        }
                                        Err(e) => {
                                            eprintln!("Error receiving from embedder: {:?}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to open embed_log stream: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to connect to Embedder: {e:?}");
                // Backoff already handled in connect_with_retry
            }
        }
    }
}

/// Retry connection with exponential back-off
/// - Keeps retrying until success or `max_reconnect_attempts`.
/// - Exponential back-off (`backoff_factor`) prevents hammering.
/// - Jitter (`retry_jitter`) prevents thundering herd problem if multiple
/// shippers restart at once.
/// - Each retry failure -> logs + sleeps before retry.
async fn connect_with_retry(
    config: &ShipperConfig,
) -> Result<EmbedderClient<Channel>, tonic::transport::Error> {
    let mut attempt = 0;
    let mut delay = Duration::from_millis(config.initial_retry_delay_ms);

    loop {
        attempt += 1;

        let endpoint = Endpoint::from_shared(config.embedder_target_addr.clone())?
            .timeout(Duration::from_millis(config.connection_timeout_ms));

        match endpoint.connect().await {
            Ok(channel) => {
                return Ok(EmbedderClient::new(channel));
            }
            Err(e) => {
                if let Some(limit) = config.max_reconnect_attempts {
                    if attempts >= limit {
                        return Err(e);
                    }
                }

                // Apply jitter to prevent thundering herd
                let jitter_factor: f64 = rand::thread_rng()
                    .gen_range(1.0 - config.retry_jitter..1.0 + config.retry_jitter);
                let sleep_duration = delay.mul_f64(jitter_factor);
                eprintln!(
                    "Retry {attempt} failed: {e:?}, sleeping {:?}",
                    sleep_duration
                );

                sleep(sleep_duration).await;

                // Exponential backoff
                delay = Duration::from_millis(
                    (delay.as_millis() as f64 * config.backoff_factor)
                        .min(config.max_retry_delay_ms as f64) as u64,
                );
            }
        }
    }
}

/// Handle responses from embedding service
fn handle_embed_response(resp: EmbedResponse) {
    // TODO: Handle response from Embedding service
}

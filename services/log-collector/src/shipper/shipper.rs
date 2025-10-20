//! Shipper - responsibility and behavior
//!
//! The Shipper is a focused component that takes *already-normalized* logs (batches `NormalizedLog`)
//! and reliably forwards them to the Embedding micro-service over gRPC.
//!
//! Key responsibilities:
//! - Maintain and manage an outbound gRPC channel/client (EmbedderClient).
//! - Opens a bi-directional streaming RPC to the embedder and stream logs.
//! - Handle reconnects with exponential backoff + jitter when Embedder is
//! unreachable.
//! - Process EmbedResponse message from the Embedding micro-service.
//! - Decouple the pipeline by exposing a simple `send(batch)` API; callers
//! do not need to handle network failures or retries.
//!
//! Important design notes:
//! - The Shipper *does not* implement large-scale buffering or durability -
//! that's the responsibility of the buffer-batcher module (which persists to
//! SQLite or keeps a bounded in-memory queue, depending on configuration).
//! - The Shipper keeps a *small* handoff channel (mpsc) to decouple the pipeline
//! so that callers can enqueue batches without blocking on network I/O.
//! - The Shipper returns a `QueueFull` error when the handoff channel is saturated;
//! the buffer-batcher should react according to its overflow policy.

use rand::Rng;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tonic::transport::{Channel, Endpoint};

use crate::buffer_batcher::log_buffer_batcher::InMemoryBuffer;
use crate::helpers::load_config::ShipperConfig;
use crate::proto::common::NormalizedLog;
use crate::proto::embedder::{EmbedResponse, embedder_client::EmbedderClient};

/// Shipper
///
/// Runtime object owned by the Collector that exposes a `send(batch)` API to enqueue
/// normalized log batches for delivery to the Embedder. The Shipper:
///     - owns the background worker that manages the gRPC connection,
///     - accepts small handoff batches via a bounded `mpsc::Sender`,
///     - does not provide durable storage - buffering/persistence lives in buffer-batcher.
///
/// Why a small mpsc channel?
///     - It decouples the caller from network ops so the pipeline continues to produce
///     batches while the Shipper performs I/O.
///     - It is intentionally *small* since the authoritative buffer and overflow policy
///     are managed by the buffer-batcher (to avoid duplicate buffering and uncontrolled
///     memory growth).
#[derive(Debug, Clone)]
pub struct Shipper {
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

    /// Enqueue a normalized batch for shipping.
    ///
    /// Behavior:
    /// - Attempts to send the batch into an internal bounded channel.
    /// - If the channel is full (sender closed or channel saturated), returns `QueueFull`.
    ///
    /// Design decisions:
    /// - Returning `QueueFull` surfaces backpressure to the caller (the buffer-batcher),
    /// which can then apply its configured overflow policy.
    /// - We do NOT block here indefinitely; upstream components decide how to behave.
    pub async fn send(&self, batch: InMemoryBuffer) -> Result<(), ShipperError> {
        self.sender
            .send(batch)
            .await
            .map_err(|_| ShipperError::QueueFull)
    }
}

/// Backgground worker loog (owns the outbound gRPC stream).
///
/// High level algorithm:
/// 1. Repeatedly attempt to connect to the Embedder using `connect_with_retry`.
/// 2. On success, open the bi-directional stream (Embedder.embed_log).
/// 3. Creates two logical loops multiplexed via `tokio::select!`:
///     - send loop: drain the mpsc receiver and push each NormalizedLog into the request stream.
///     - recv loop: read EmbedResponse messages from the response stream and handle them.
/// 4. If either loop fails (send error, response error, stream closed), break the inner loop
/// and retry the connection (connect_with_retry).
///
/// Failure semantics:
/// - Transient network errors will cause reconnect attempts with backoff.
/// - Persistent failures eventually bubble up as logs/metrics; the buffer-batcher guarantees
/// durability if configured (SQLite).
///
/// Note: The worker keeps the Embedder connection transiently alive - if the Embedder
/// closes the stream the worker will tear it down and re-establish it.
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

/// Connect to the Embedder with exponential backoff + jitter
///
/// Policy:
/// - Start with `initial_retry_delay_ms`.
/// - Multiply delay by `backoff_factor` on each failed attempt, capping at `max_retry_delay_ms`.
/// - Apply jitter to randomize retry timing (prevents thundering herd problem).
/// - Respect `max_reconnect_attempts` (if set); otherwise retry indefinitely.
///
/// Observability:
/// - Emit logs or metrics on each failed attempt and on successful re-connect.
/// - Timeouts for the connection attempt come from `connection_timeout_ms`.
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

/// Handle EmbedResponse messages from the Embedder micro-service.
///
/// Responsibilities:
/// - Persist or forward embedding vectors to storage/caches if needed.
/// - Increment metrics (embeddings_received_total).
/// - Optionally correlate the response with the original NormalizedLog (if IDs are used).
///
/// Note: The Shipper should not perform heavy synchronous work here; prefer batching updates
/// or offloading to another task to keep the receive loop responsive.
fn handle_embed_response(resp: EmbedResponse) {
    // TODO: Handle response from Embedding service
}

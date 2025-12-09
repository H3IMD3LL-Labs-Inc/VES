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

// Local crates
use crate::{
    buffer_batcher::log_buffer_batcher::InMemoryBuffer,
    helpers::converters::*,
    helpers::load_config::ShipperConfig,
    proto::common::NormalizedLog as ProtoNormalizedLog,
    proto::embedder::{EmbedResponse, embedder_client::EmbedderClient},
};

// External crates
use rand::Rng;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};
use tracing::instrument;

/// Shipper
///
/// Runtime object owned by the Log Collector that exposes a `send(batch: InMemoryBuffer)` API to enqueue
/// normalized log batches for delivery to the Log Embedder micro-service. The Shipper:
///     - owns the background worker that manages the gRPC connection,
///     - accepts small handoff batches via a bounded `mpsc::Sender`,
///     - does not provide durable storage - buffering/persistence lives in buffer-batcher module.
///
/// Why a small mpsc channel?
///     - It decouples the caller from network ops so the pipeline continues to produce
///     batches while the Shipper performs I/O.
///     - It is intentionally *small* since the authoritative buffer and overflow policy
///     are managed by the buffer-batcher (to avoid duplicate buffering and uncontrolled
///     memory growth).
#[derive(Debug)]
pub struct Shipper {
    sender: mpsc::Sender<InMemoryBuffer>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker_handle: Option<JoinHandle<()>>,
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
    #[instrument(
        name = "core_agent_shipper::create",
        target = "shipper::shipper::Shipper",
        skip_all,
        level = "debug"
    )]
    pub async fn new(config: ShipperConfig) -> Self {
        tracing::debug!("Creating Shipper producer and worker channels");
        // Small channel to decouple producer and worker
        let (tx, rx) = mpsc::channel(1); // maybe 5-10, for some slack
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tracing::debug!("Creating asynchronous running Shipper worker task with JoinHandle");
        let handle = tokio::spawn(run_worker(config.clone(), rx, shutdown_rx));

        tracing::debug!("Creating core agent Shipper object");
        Self {
            sender: tx,
            shutdown_tx: Some(shutdown_tx),
            worker_handle: Some(handle),
        }
    }

    /// Enqueue a uniformly structured data batch for shipping.
    ///
    /// Behavior:
    /// - Attempts to send the batch into an internal bounded channel.
    /// - If the channel is full (sender closed or channel saturated), returns `QueueFull`.
    ///
    /// Design decisions:
    /// - Returning `QueueFull` surfaces backpressure to the caller (the buffer-batcher),
    /// which can then apply its configured overflow policy.
    /// - We do NOT block here indefinitely; upstream components decide how to behave.
    #[instrument(
        name = "core_agent_shipper::send",
        target = "shipper::shipper::Shipper",
        skip_all,
        level = "debug"
    )]
    pub async fn send(&self, batch: InMemoryBuffer) -> Result<(), ShipperError> {
        tracing::debug!(
            data_queue = %batch.queue.len(),
            data_batch_size = %batch.batch_size,
            "Enqueing uniformly structured data to Shipper internal sender channel"
        );
        self.sender
            .send(batch)
            .await
            .map_err(|_| ShipperError::QueueFull)
    }

    /// Gracefully shutdown the Shipper worker
    ///
    /// Sends the shutdown signal to the worker and waits for it to finish.
    #[instrument(
        name = "core_agent_shipper::send",
        target = "shipper::shipper::Shipper",
        skip_all,
        level = "debug"
    )]
    pub async fn shutdown(&mut self) {
        tracing::debug!("Sending shutdown signal to Shipper worker channel");
        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        tracing::debug!("Waiting for Shipper worker channel task to finish work");
        // Wait for the worker to finish
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.await;
        }

        tracing::debug!("Shipper worker channel gracefully shutdown successfully");
    }
}

/// Background worker loop (owns the outbound gRPC stream).
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
#[instrument(
    name = "core_agent_shipper::run_worker",
    target = "shipper::shipper::Shipper",
    skip_all,
    level = "debug"
)]
async fn run_worker(
    config: ShipperConfig,
    mut rx: mpsc::Receiver<InMemoryBuffer>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    tracing::debug!("Running asynchronous Shipper worker task background loop");
    loop {
        // Exit outer loop if shutdown triggered before connecting is triggered
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::error!(
                    shutdown_signal = ?shutdown_rx,
                    "Exiting Shipper worker task background loop, shutdown signal received before connection. Exiting worker"
                );
                return;
            }
            conn_result = connect_with_retry(&config) => {
                match conn_result {
                    Ok(mut client) => {
                        // setup gRPC send channel
                        let (out_tx, out_rx) = mpsc::channel::<ProtoNormalizedLog>(10);
                        let request_stream = ReceiverStream::new(out_rx);
                        tracing::debug!(
                            channel_request_stream = ?request_stream,
                            "Setting up asynchronous Shipper worker task background loop gRPC send channel"
                        );

                        match client.embed_log(request_stream).await {
                            Ok(response) => {
                                let mut response_stream = response.into_inner();
                                tracing::debug!(
                                    "Connection to Vector Embedding Engine established. Streaming uniformly structured observability data"
                                );

                                loop {
                                    tokio::select! {
                                        // Normal batch forwarding
                                        maybe_batch = rx.recv() => {
                                            match maybe_batch {
                                                Some(batch) => {
                                                    for log in batch.queue {
                                                        let protobuf_log: ProtoNormalizedLog = log.into();
                                                        if out_tx.send(protobuf_log).await.is_err() {
                                                            tracing::error!(
                                                                "Vector Embedding Engine gRPC send channel stream closed while sending some uniformly structured observability data"
                                                            );
                                                            break;
                                                        }
                                                    }
                                                }
                                                None => {
                                                    tracing::error!(
                                                        "Worker background loop task channel sender channel closed, exiting worker"
                                                    );
                                                    // Channel closed, nothing more to send
                                                    return;
                                                }
                                            }
                                        }

                                        // Receive messages from Embedder
                                        maybe_resp = response_stream.message() => {
                                            match maybe_resp.transpose() {
                                                Some(Ok(embed_resp)) => handle_embed_response(embed_resp),
                                                Some(Err(e)) => {
                                                    tracing::error!(
                                                        error = %e,
                                                        "Error receiving Vector Embedding Engine responses in gRPC channel, stream may be unhealthy"
                                                    );
                                                    break;
                                                }
                                                None => {
                                                    tracing::debug!(
                                                        "Vector Embedding Engine closed its response stream in the gRPC channel"
                                                    );
                                                    break;
                                                }
                                            }
                                        }

                                        // Shutdown signal received
                                        _ = &mut shutdown_rx => {
                                            tracing::debug!(
                                                shutdown_signal = ?shutdown_rx,
                                                "Shutdown signal received by Shipper worker task background loop while connected to Vector Embedding Engine, draining uniformly structured data batches in the channel"
                                            );

                                            // Drain all remaining batches from rx
                                            while let Ok(batch) = rx.try_recv() {
                                                for log in batch.queue {
                                                    let protobuf_log: ProtoNormalizedLog = log.into();
                                                    let _ = out_tx.send(protobuf_log).await;
                                                }
                                            }

                                            tracing::debug!(
                                                "Pending uniformly structured data batches sent to Vector Embedding Engine. Closing Shipper worker task and background loop"
                                            );
                                        }
                                    }
                                }
                            }

                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "Shipper worker task background loop failed to open embed_log gRPC stream"
                                );
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }

                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "Shipper worker task background loop to establish connection to Vector Embedding Engine"
                        );
                        // Retry handled in connect_with_retry()
                    }
                }
            }
        }
    }
}

/// Attempt connection establishment to Vector Embedding Engine with exponential backoff + jitter logic
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
#[instrument(
    name = "core_agent_shipper::connection_with_retry",
    target = "shipper::shipper::Shipper",
    skip_all,
    level = "debug"
)]
async fn connect_with_retry(
    config: &ShipperConfig,
) -> Result<EmbedderClient<Channel>, tonic::transport::Error> {
    let mut attempts = 0;
    let mut delay = Duration::from_millis(config.initial_retry_delay_ms);

    tracing::debug!(
        "Attempting to connect to Vector Embedding Engine using exponential backoff and jitter logic"
    );
    loop {
        attempts += 1;

        let endpoint = Endpoint::from_shared(config.embedder_target_addr.clone())?
            .timeout(Duration::from_millis(config.connection_timeout_ms));
        tracing::info!(
            embedding_engine_addr = ?endpoint,
        );

        tracing::debug!(
            embedding_engine_addr = ?endpoint,
            "Creating HTTP/2 connection channel to Vector Embedding Engine gRPC server using addr endpoint config"
        );
        match endpoint.connect().await {
            Ok(channel) => {
                tracing::info!(
                    "Successfully created HTTP/2 connection channel to Vector Embedding Engine gRPC server"
                );
                return Ok(EmbedderClient::new(channel));
            }
            Err(e) => {
                if let Some(reconnect_attempts_limit) = config.max_reconnect_attempts {
                    if attempts >= reconnect_attempts_limit {
                        tracing::error!(
                            error = %e,
                            "Exceeded configured max_reconnect_attempts to create HTTP/2 connection channel to Vector Embedding Engine gRPC server"
                        );
                        return Err(e);
                    }
                }

                // Apply jitter to prevent thundering herd
                let jitter_factor: f64 = rand::thread_rng()
                    .gen_range(1.0 - config.retry_jitter..1.0 + config.retry_jitter);
                tracing::debug!(
                    connection_attempt_jitter = %jitter_factor,
                    "Applying jitter to Vector Embedding Engine HTTP/2 connection channel creation attempt"
                );

                let sleep_duration = delay.mul_f64(jitter_factor);
                tracing::error!(
                    error = %e,
                    sleep_duration = ?sleep_duration,
                    "HTTP/2 connection channel creation retries {attempts} failed sleeping"
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

// External crates
use tokio::sync::broadcast;
use tracing::instrument;

/// Global shutdown manager, built on-top of a broadcast channel
///
/// - `tx` is cloned by each Log Collector component.
/// - Each Log Collector component calls `.subscribe()` to get its own receiver.
/// - Calling `.trigger()` sends the shutdown signal to all components.
///
/// # Design choice:
/// Unlike `Notify` or `tokio::sync::watch`, `broadcast` is a multi-consumer one-
/// shot signalling primitive:
/// - All receivers get the same message.
/// - New subscribers can be added at runtime.
/// - Each receiver owns its independent read cursor.
/// - Integrates cleanly with `tokio::select!` for concurrent responsiveness.
#[derive(Debug, Clone)]
pub struct Shutdown {
    pub tx: broadcast::Sender<()>,
}

impl Shutdown {
    /// Creates a new shutdown broadcast channel.
    /// A small buffer size is sufficient since only one message is sent.
    #[instrument(
        name = "ves_shutdown_channel",
        target = "helpers::shutdown",
        level = "trace"
    )]
    pub fn new() -> Self {
        tracing::trace!("Creating new global shutdown channel");
        let (tx, rx) = broadcast::channel(16);
        Self { tx }
    }

    /// Returns a new receiver handle for a Log Collector component
    #[instrument(
        name = "ves_shutdown_subscriber",
        target = "helpers::shutdown",
        level = "trace"
    )]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        tracing::trace!("Shutdown subscriber created for shutdown channel");
        self.tx.subscribe()
    }

    /// Trigger shutdown event, notifying all components with Receivers
    #[instrument(
        name = "ves_shutdown_trigger",
        target = "helpers::shutdown",
        level = "trace"
    )]
    pub fn trigger(&self) {
        tracing::trace!("Shutdown triggered, notify created global shutdown channel subscribers");
        let _ = self.tx.send(());
    }

    /// Wait for a shutdown signal (used in main runtime or top-level managers).
    /// Simply blocks until `.trigger()` is called.
    #[instrument(
        name = "ves_shutdown_waiter",
        target = "helpers::shutdown",
        level = "trace"
    )]
    pub async fn wait_for_shutdown(&self) {
        tracing::trace!("Waiting for shutdown signal");
        let mut rx = self.tx.subscribe();
        let _ = rx.recv().await;
        tracing::trace!("Shutdown signal received, VES program returning");
    }
}

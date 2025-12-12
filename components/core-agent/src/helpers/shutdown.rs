// External crates
use tokio::sync::broadcast;
use tracing::instrument;

/// Global shutdown manager, built on-top of a broadcast channel
#[derive(Debug, Clone)]
pub struct Shutdown {
    pub tx: broadcast::Sender<()>,
}

impl Shutdown {
    /// Creates a new shutdown broadcast channel.
    /// A small buffer size is sufficient since only one message is sent.
    #[instrument(
        name = "core_agent_shutdown_broadcaster",
        target = "helpers::shutdown",
        level = "debug"
    )]
    pub fn new() -> Self {
        tracing::info!("Creating new global shutdown broadcaster channel");
        let (tx, _rx) = broadcast::channel(1);
        Self { tx }
    }

    /// Returns a new receiver handle for a Log Collector component
    #[instrument(
        name = "core_agent_shutdown_subscriber",
        target = "helpers::shutdown",
        level = "debug"
    )]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        tracing::info!("Shutdown subscriber created for shutdown broadcast channel Sender");
        self.tx.subscribe()
    }

    /// Trigger shutdown event, notifying all components with Receivers
    #[instrument(
        name = "core_agent_shutdown_trigger",
        target = "helpers::shutdown",
        level = "debug"
    )]
    pub fn trigger(&self) {
        tracing::info!("Shutdown triggered, notify created global shutdown broadcast channel Sender subscribers");
        let _ = self.tx.send(());
    }

    /// Wait for a shutdown signal (used in main runtime or top-level managers).
    /// Simply blocks until `.trigger()` is called.
    #[instrument(
        name = "core_agent_shutdown_waiter",
        target = "helpers::shutdown",
        level = "debug"
    )]
    pub async fn wait_for_shutdown(&self) {
        tracing::info!("Waiting for shutdown signal");
        let mut rx = self.tx.subscribe();
        let _ = rx.recv().await;
        tracing::info!("Shutdown signal received, VES program returning");
    }
}

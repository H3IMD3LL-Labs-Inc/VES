use tokio::sync::broadcast;

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
#[derive(Clone)]
pub struct Shutdown {
    pub tx: broadcast::Sender<()>,
}

impl Shutdown {
    /// Creates a new shutdown broadcast channel.
    /// A small buffer size is sufficient since only one message is sent.
    pub fn new() -> Self {
        let (tx, rx) = broadcast::channel(16);
        Self { tx }
    }

    /// Returns a new receiver handle for a Log Collector component
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    /// Trigger shutdown event, notifying all components with Receivers
    pub fn trigger(&self) {
        let _ = self.tx.send(());
    }

    /// Wait for a shutdown signal (used in main runtime or top-level managers).
    /// Simply blocks until `.trigger()` is called.
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.tx.subscribe();
        let _ = rx.recv().await;
    }
}

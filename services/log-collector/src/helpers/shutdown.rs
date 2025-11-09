/// ======================================================================
///                                 GOALS
/// ======================================================================
///
/// 1. No ongoing work is abruptly interrupted
/// 2. All pending log data is flushed or persisted
/// 3. All resources are released safely
/// 4. The shutdown is co-ordinated across all Log Collector subsystems
///
/// ======================================================================
///                             BUILDING BLOCKS
/// ======================================================================
///
/// 1. Shutdown signal broadcaster
/// - A central shared object (like Shutdown) that can notify multiple async
/// tasks at once that it's time to stop.
///
/// 2. A signal listener
/// - A background task that listens for system signals like: CTRL+C(interact
/// ive stop), SIGTERM(container or systemd stop), SIGINT(manual kill -2).
/// Once detected, it triggers the broadcaster.
///
/// 3. Co-operative shutdown handling in each Log Collector component
/// - Each long-running component needs to co-operate. They shouldn't just
/// run infinite loops. They should periodically check for a shutdown event
/// and exit gracefully when told to. This ensures soft stop rather than
/// hard termination.
use tokio::sync::broadcast;

/// Global shutdown manager, built on-top of a broadcast channel
///
/// - `tx` is cloned by each Log Collector component.
/// - Each Log Collector component calls `.subscribe()` to get its own receiver.
/// - Calling `.trigger()` sends the shutdown signal to all components.
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
}

## Log Watcher
Ensure local log watcher has the following:
1. Config-driven.
2. Properly connected to the shared pipeline (parser, buffer_batcher, shipper).
3. Resilient (checkpoint recovery, cleanup, async concurrency).

---

### TODO List (in order of implementation)

1️⃣  Accept Config and Service References
- Add fields to `LogWatcher` for:
  - `poll_interval_ms` (from config)
  - `recursive` (from config)
  - A reference (`Arc<LogCollectorService`>) for sending logs to the shared pipeline.
    > This allows the watcher to hand off logs directly into the shared pipeline instead of running in isolation.
- Reason: `LogWatcher` is created in main.rs using
```rust
LogWatcher::new_watcher(log_dir, checkpoint_path, Arc<LogCollectorService>)
```
so the watcher has access to the rest of the pipeline.

2️⃣  Fix Checkpoint Recovery
- Complete `load_checkpoint()` to:
  - Read all files from checkpoint.
  - Restore offsets to any existing tailers.
  - Detect rotated or missing files and clean them up.
- Reason: After restarts, the watcher should resume where it left off instead of re-reading entire logs.

3️⃣  Implement Graceful Shutdown for Tailers
- Introduce a shutdown signal or `tokio::sync::mpsc` channel.
- Pass it into each tailer so that if the watcher is stopped, all tailers gracefully close their file handles and flush remaining offsets to checkpoint.
- Reason: Prevent runaway tailer tasks and corrupted checkpoints during restart.

4️⃣  Unify Tailer Spawning Behavior
- Currently, `discover_initial_files()` spawns tailers with `tokio::spawn`, but `handle_new_file()` awaits inline.
- Make this consistent:
  - Always spawn new tailers via `tokio::spawn`.
  - Use `Arc<LogCollectorService>` to push tailer output into the shared pipeline.
- Reason: prevent blocking the watcher event loop when tailers open large files.

5️⃣  Connect Tailer -> Parser -> Buffer-Batcher -> Shipper Flow
- Inside each `Tailer`, when new log lines are read:
  - Send them through the shared pipeline.
- Reason: Move away from a standalone Tailer to a fully integrated local ingestion path.

6️⃣  Add Backpressure handling
- Replace `tx.blocking_send(event)` with:
  - `tx.try_send(event)` or
  - `tokio::spawn` a lightweight task per event to prevent blocking.
- Use bounded channels with clear overflow behavior (`drop_oldest`, `drop_newest`, etc.) from `[buffer]` config.
- Reason: Prevents high-volume log events from stalling the watcher.

7️⃣  Improve Checkpoint Persistence
- Move checkpoint saving into a background task:
  - Periodically flush in-memory offsets to disk.
  - Don't write to disk on every log line or event - that's expensive.
- Add a timer like `save_interval_ms` in `[watcher]` config.

8️⃣  Error Handling & Recovery
- Wrap all filesystem operations (`metadata`, `read_dir`, etc.) in `anyhow::Context`.
- Log failures clearly with log levels.
- If a tailer crashes (e.g., file deleted mid-read), automatically remove it from `active_files` and update checkpoint.

9️⃣  Make Recursive Watcher Optional
- Respect `recursive = true` from config:
  - If true, use `RecursiveMode::Recursive`
  - If false, `RecursiveMode::NonRecursive`
- Useful for Kubernetes pod logs or deeply nested container directories.

🔟 Observability & Metrics (optional)
- Add instrumentation points (with `tracing` or prometheus metrics):
  - Number of active tailers
  - Files discovered
  - Checkpoint save duration
  - Channel backlog depth

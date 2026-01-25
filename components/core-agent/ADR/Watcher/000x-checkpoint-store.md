# ADR: Introducing CheckpointStore for Watcher Durability

## Status: *Pending*

## Context
A Watcher tracks per-file state (inode, path & offset) in order for a [Tailer](/components/core-agent/src/tailer/models.rs) to tail data files and correctly resume processing after restarts.

Originally, this state was held entirely in memory using a `Checkpoint` struct backed by a `HashMap<Inode, FileState>. This approach fails to meet the following requirements:
- The Core Agent must survive crashes and unclean shutdowns (e.g., SIGKILL).
- File offsets must not be lost or corrupted across restarts.
- Restarting the agent must not cause duplicated or skipped data.
- Checkpoint updates must be atomic and crash-safe.

Persisting the entire in-memory checkpoint periodically (e.g, via JSON or bincode snapshots, etc.) was considered but was not quite right because of:
- Risk of torn writes on crashes.
- Whole-state rewrites for small updates.
- Increased complexity in guaranteeing correctness.

## Decision
Introduce a `CheckpointStore` abstraction that defines the durability boundary for a Watcher's checkpoint stats.
- `CheckpointStore` is responsible for persisting and recovering checkpoint state across crashes and restarts.
- `Checkpoint` remains as an in-memory view of the current state used by a Watcher's logic.
- All mutations to checkpoint state must be written to `CheckpointStore` before being reflected in memory.

LMDB (via the [heed3](https://crates.io/crates/heed3) crate) is chosen as the initial implementation of `CheckpointStore` due to its ACID guarantees, crash safety and suitability for single-writer, read-heavy workloads.

## Invariants
1. `CheckpointStore` is the source of truth for checkpoint state.
2. Any state present in `CheckpointStore` must survive process crashes.
3. In-memory `Checkpoint` state may be discarded and reconstructed at any time.
4. Persistent writes must occur before in-memory updates.
5. Watcher logic must not depend on LMDB or any storage-specific details.

Violating these risks data duplication, data loss or inconsistent restart behavior.

## Outcomes
### Positive
- Crash-safe recovery of data file offsets and inode state.
- Deterministic restart behavior.
- Clear separation between domain logic and persistence.
- Ability to test Watcher logic without disk or LMDB.
- Future storage backends can be added without modifying Watcher logic.

### Negative
- Slight increase in architectural complexity.
- Requires careful ordering of writes.
- Introduces dependency on an embedded database (LMDB).

## Follow-up work
- Implement `CheckpointStore` using [heed3](https://crates.io/crates/heed3).
- Define commit frequency and batching strategy for offset updates.
- Add integration tests that simulate crash and restart scenarios.

## Alternatives Considered & Rejected
1. Periodic snapshot file (JSON, bincode)
2. SQLite
3. RocksDB
4. In-memory only

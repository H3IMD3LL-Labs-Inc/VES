```
                      ┌──────────────────────────────┐
                      │ LogCollectorService          │
                      │                              │
                      │  ├─ watcher (local logs)     │
                      │  ├─ tailer                   │
                      │  ├─ parser                   │
                      │  ├─ buffer_batcher           │
                      │  ├─ shipper                  │
                      │  └─ gRPC (remote logs)       │
                      └──────────┬───────────────────┘
                                 │
        ┌────────────────────────┼────────────────────────┐
        │                                                 │
        ▼                                                 ▼
 Local Filesystem                                  Remote gRPC Clients
 (watcher → tailer)                                (stream_log())
        │                                                 │
        ▼                                                 ▼
      parser                                           parser
        │                                                 │
        ▼                                                 ▼
  log_buffer_batcher                              log_buffer_batcher
        │                                                 │
        ▼                                                 ▼
      shipper                                           shipper
        │                                                 │
        └──────────────→ Embedder / Storage Service ←──────┘
```
The Log Collector is a unified ingestion system, it's built to accept logs from various
sources but process them through the same internal pipeline. There are two main ways logs enter the system:
a) Remote Clients (Network-based ingestion)
- Remote applications or services (micro-services, containers, agents) connect to the Log Collector over the network.
- They send logs via gRPC streaming, calling the RPC endpoint defined in collector.proto.
- Inside server.rs, the stream_log() method is invoked automatically by the gRPC framework (Tonic) when a remote client starts sending logs.
- The request contains a continuous stream of RawLog messages, which the server processes as they arrive.
- Each log goes through the same internal pipeline:

```
RawLog
   ↓
parser.rs (→ NormalizedLog)
   ↓
log_buffer_batcher.rs (batching, buffer flush)
   ↓
shipper.rs (send to embedder or external sink)
```

b) Local FileSystem (Agent-based ingestion)
- These are logs already exist on the same machine where the Log Collector is running.
- For example;
  - Application logs written to `/var/log/<app_name>.log`
  - System logs written to `/var/log/syslog`
  - Any files being continuously appended (like web server access logs).
- This flow is completely local - no gRPC connection is involved.
- It uses the watcher-tailer modules, to watch and tail files, and the other modules to process the logs.

⚖️ Why Two Log Sources?
```
| Purpose              | Remote Clients (gRPC)                             | Local Filesystem |
| -------------------- | ------------------------------------------------- | ---------------- |
| Where logs come from | External apps                                     | Local files      |
| How logs arrive      | Over the network                                  | From disk        |
| Who initiates        | Client                                            | Log Collector    |
| Common path          | Both go through parser → buffer_batcher → shipper |                  |
```

Both eventually generate NormalizedLog, batch them, and send them off to the Embedding micro-service. The only difference is how they get into the system. Even though triggers are different (filesystem vs network), they both converge into the same internal architecture - this ensures:
✅ Consistency - all logs (no matter origin) are parsed and normalized in the same way.
✅ Re-usability - one parser, one buffer, one shipper - no duplicated logic.
✅ Extensibility - can later include; Kafka, Journald, etc. as more sources - they'll all just feed the same core modules.

```
                ┌────────────────────────────┐
                │     LogCollectorService    │
                │ (core processing pipeline) │
                │ watcher → tailer → parser  │
                │ buffer_batcher → shipper   │
                └────────────┬───────────────┘
                             │
          ┌──────────────────┴──────────────────┐
          │                                     │
   🧠 Local Filesystem                   🌐 Network (gRPC)
   watcher.rs + tailer.rs                server.rs + gRPC Client
   Automatically reads log files         Apps push logs over gRPC
   No code changes for user              Requires SDK or .proto import
```
---
1. Local FileSystem Ingestion
- This mode runs automatically inside the log collector service. It watches local log files, tails them for new log lines, parses, buffer-batches, and ships them.

How It Works Internally
- Watcher: watches configured directories for log files. Triggers tailer whenever new files or changes are detected.
- Tailer: tails each file line-by-line. Sends each raw line to the Parser.
- Parser: Converts raw text lines -> structured logs (NormalizedLog).
- BufferBatcher: Temporarily buffers and batches logs, based on user configuration.
- Shipper: Sends batched normalized logs to the embedding micro-service.
---

Example Flow
```
/var/log/app.log
   ↓ watcher detects changes
tailer reads lines
   ↓
parser parses log lines
   ↓
buffer_batcher accumulates logs
   ↓
shipper sends logs to embedder
```
---

- The Local FileSystem Mode allows the Log Collector to automatically detect and collect logs from specified file paths or directories. This requires no application-level integration. It's ideal for system logs, container logs, or legacy applications. It's also suitable for applications that don't have a dedicated logging system.
---

2. Network (gRPC) Ingestion
- This mode allows remote applications or services to stream logs directly to the log collector over the network via gRPC. This acts as a log-forwarder SDK.

How It Works Internally
- Client (inside user's application): Creates a LogCollectorClient (generated from .proto). Streams RawLog messages to the log collector.
- Server: `server.rs` implements `stream_log()`. For each incoming log, it performs necessary processing. Sends back acknowledgements via a stream of CollectResponse.
---

Example Flow
```
user_app.rs → LogCollectorClient → gRPC → LogCollectorService.stream_log()
   ↓
parser parses log lines
   ↓
buffer_batcher accumulates logs
   ↓
shipper sends logs to embedder
```
---

- The Network Mode allows applications and micro-services to send logs directly to the Log Collector over gRPC. This mode enables structured logging, real-time ingestion, and richer metadata. Applications can integrate easily using provided SDKs or the generated gRPC client stubs.
---

3. Summary Table
```
| Mode                | Purpose                     | How Logs Enter        | Integration Effort   | Ideal Use Case            |
| ------------------- | --------------------------- | --------------------- | -------------------- | ------------------------- |
| 🧠 Local Filesystem | Automatic local ingestion   | File watcher → Tailer | None                 | Legacy apps, system logs  |
| 🌐 Network (gRPC)   | Remote structured ingestion | gRPC stream from apps | Minimal (SDK import) | Microservices, containers |
```
---

Log Collector: Log Collection Design
- The log collector is designed to collect logs both locally and remotely;
  - Some users will run it as a local agent, automatically reading log files as they are generated.
  - Others will send logs remotely via gRPC, using an SDK or .proto client.
- Both modes exist in the same process, using the same underlying core logic.
---

1. Design Goals
✅ One binary handles both modes.
✅ Each mode can be enabled/disabled independently (via config).
✅ Both modes share the same core logic.
✅ All modules(core logic) stays decoupled and composable.

2. Architecture Overview
```
main.rs
│
├── Loads config
│
├── Creates shared LogCollectorService instance
│
├── Spawns Local Watcher (optional)
│     └── watcher.rs → tailer.rs → parser.rs → buffer_batcher.rs → shipper.rs
│
└── Starts gRPC Server (optional)
      └── server.rs → stream_log() → parser.rs → buffer_batcher.rs → shipper.rs
```

- The configuration file allows users to configure log ingestion modes on the fly.
---

3. How These Modes Run At Runtime
```
| Mode            | Enabled in Config                               | Process Behavior                                           |
| --------------- | ----------------------------------------------- | ---------------------------------------------------------- |
| 🧠 Local only   | `enable_local = true`, `enable_network = false` | Runs watcher → tailer internally, no network socket opened |
| 🌐 Network only | `enable_local = false`, `enable_network = true` | Runs gRPC server only, accepts remote logs                 |
| 🧠🌐 Both       | Both `true`                                     | Runs both concurrently (parallel tokio tasks)              |
```
---

4. Shared State Between Modes
- Because both modes need to access the same pipeline modules, LogCollectorService in Arc. Internally, each module that mutates state (i.e, buffer-batcher) can use `tokio::sync::Mutex` or `RwLock`.
---

6. Optional: Add CLI Flags for Quick Overrides To Configuration
- Complement the config file based configuration with CLI flags using `clap`.
---

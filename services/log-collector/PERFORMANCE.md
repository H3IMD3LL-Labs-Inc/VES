# Sub-Millisecond Service Latency

Goal:
> Provide code-level and host OS/runtime optimizations only. Network/provider-level optimizations (dedicated instances, RDMA, NIC offloads) are out of scope at this point.

## Table Of Contents
1. Quick checklist
2. High-level plan and objectives
3. Build and release configurations (Rust/Cargo)
4. Runtime & Tokio tuning
5. gRPC (Tonic) transport tuning - server + client knobs
6. Serialization & message design (prost / FlatBuffers / zero-copy)
7. Memory managements, buffers, and allocation pools
8. Concurrency, locking, and async patterns to avoid
9. CPU affinity, NUMA, and thread pinning
10. Observability: metrics, tracing, and high-precision timing
11. Benchmarking & performance testing harnesses
12. Profiling * cold-start / tail latency diagnosis
13. CI / regression testing for latency guarantees
14. Common pitfalls & troubleshooting checklist
15. Future (advanced) options to consider for "VES Cloud"

---

### 1) Quick Checklist
Apply these and measure:
- Build in `--release` with `-C target-cpu=native`, `lto=true`, `codegen-units=1`. (See section 3).
- Use `TcpSocket` + `socket.set_nodelay(true)` for all connections (disable Nagle).
- Pre-allocate `BytesMut` buffers; reuse them (buffer pool).
- Replace JSON/serde with `prost` (binary protobuf) or FlatBuffers for hot paths.
- Reduce `.await` points in hot paths; avoid `.await` inside locks.
- Pin runtime threads to dedicated cores (`core_affinity`).
- Measure tail latency with hdrhistogram; add automated p50/p95/p99/p999 checks.

---

### 2) High-level Plan & Objectives (single AZ code-only)
- Objectives: get RPC latency (round trip or request-handler completion) into 0.2-0.8 ms in the same AZ through software optimizations.
- Primary constraints: keep app behavior/functionality identical; only change implementation details that won't alter external APIs.
- Success metrics:
  - Median latency within target.
  - 99.9th percentile (p999) within ~2x median (tail control).
  - Low variabiltiy (low jitter) over stress tests and 60+ min runs.
- Workflow: measure -> change (one class of change at a time) -> measure again -> promote to CI guardrails.

---

### 3) Build & Release Configuration (Cargo, Rustflags)
- Optimize compilation for speed and predictable codepaths in Cargo.toml:
```toml
[profile.release]
opt-level = 3            # maximize optimization for speed
lto = true               # link-time optimization
codegen-units = 1        # enable more optimization opportunities
debug = false
panic = "abort"
overflow-checks = false
```

In .cargo/config.toml / RUSTFLAGS (for target CPU and linker)
```toml
[build]
rustflags = ["-C", "target-cpu=native", "-C", "link-arg=-s"]
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native", "-C", "prefer-dynamic"]
```

- Ensure debug symbols are stripped in production builds (link-arg `-S`) keeping them in CI artifacts for profiling builds.

---

### 4) Tokio Runtime Tuning
- Tokio defaults are good for throughput but not for deterministic low latency.
- Create a dedicated, tuned runtime for latency-critical services:
```rust
use tokio::runtime::Builder;

let rt = Builder::new_multi_thread()
    .worker_threads(4)                  // match number of logical cores for this role
    .thread_name("ves-lowlatency")      // helpful for debugging & profiling
    .enable_all()                       // enables IO, time, etc.
    .max_blocking_threads(1)            // limit blocking pool size
    .build()
    .unwrap();

rt.block_on(async {
    // run tonic server / latency-critical tasks here
});
```
- Guidelines;
  - `new_multi_thread()`: -> Creates a multi-threaded runtime, which is needed for concurrent async tasks and high I/O.
  - `worker_threads()`: -> Number of worker threads(event loop executors), should be matched to number of physical cores on the machine running this service. Oversubscribing = thread contention and unpredictable latency.
  - `thread_name()`: -> Sets thread names, which makes tracing and profiling readable.
  - `enable_all()`: -> Enables IO, time, and signal drivers, which is needed for networking and async timers.
  - `max_blocking_threads()`: -> Number of blocking threads, this reduces scheduler jitter; prevents blocking calls (like file I/O) from interfering with hot async path.
  - `block_on()`: -> Runs async tasks inside the runtime, ensuring low latency critical code stays isolated from global background tasks.

- Rationals;
  -
---

### 5) gRPC (Tonic) transport tuning — server & client
- Tonic sits on hyper/http2. Tune both gRPC and lower transport.

- Server setup example (Tonic / TcpSocket):
```rust
use tonic::transport::Server;
use tokio::net::TcpSocket;
use std::time::Duration;

let socket = TcpSocket::new_v4()?;
socket.set_reuseaddr(true)?;
socket.set_nodelay(true)?; // TCP_NODELAY => disable Nagle
// optionally set socket buffer sizes (SO_SNDBUF / SO_RCVBUF)
let listener = socket.bind(addr)?.listen(128);

Server::builder()
    .http2_keepalive_interval(Some(Duration::from_secs(5)))
    .http2_keepalive_timeout(Some(Duration::from_secs(5)))
    .initial_stream_window_size(Some(1024 * 1024))   // increase window sizes
    .initial_connection_window_size(Some(1024 * 1024))
    .add_service(MyServiceServer::new(my_service))
    .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
    .await?;
```

- Client tuning:
  - Set `TCP_NODELAY` on client sockets.
  - If using connection pooling, keep connections warm; avoid frequent connect/teardown cycles.
  - Consider a long-lived HTTP/2 session per service pair to remove TCP/TLS handshake effects.

- Transport-level tips:
  - Increase HTTP/2 initial window sizes to avoid frequent `WINDOW_UPDATE` frames for small-burst workloads.
  - Keep small message sizes — avoid large, fragmented frames.
  - Consider gRPC with proto `prost` (binary) to reduce encode/decode time.

- Consider QUIC in the future (gRPC over HTTP/3) — eliminates head-of-line blocking and can improve latency in some scenarios. Not yet mainstream in Tonic; consider quinn/h3 stacks later.

---

### 6) Serialization & Message Design
- Serialization is often the single biggest per-request CPU cost.

- Use `prost` (protobuf) or FlatBuffers:
  - `prost` is simple and fast for small messages.
  - FlatBuffers offers true zero-copy deserialization for some use cases, but requires fixed schemas and careful management.

- Design messages for speed:
  - Keep messages flat: avoid deep nesting and many repeated fields.
  - Use fixed-size fields where possible (e.g., `u64`, `f64`) — decoder work is lower.
  - Keep message sizes small (optimally < 1KB for latency-sensitive RPCs).

- Pre-allocate encoding buffers;
```rust
let mut buf = Vec::with_capacity(256);
message.encode(&mut buf).unwrap();
```

- Always reserve capacity to avoid re-allocation for repeated operations. Some zero-copy tips;
  - Use `bytes::Bytes` and `bytes::BytesMut`.
  - For responses, try to send `Bytes` directly if the transport accepts it — avoid copying from Vec to slice to wire.

---

### 7) Memory Management, Buffer Pools, and Allocators
- Allocators + GC (not in Rust) are expensive. In Rust, control allocations strictly.

- Use pooled buffers, a simple pool pattern would look like this;
```rust
use bytes::BytesMut;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

struct BufferPool {
    inner: Mutex<VecDeque<BytesMut>>,
    buf_size: usize,
}

impl BufferPool {
    fn new(buf_size: usize, capacity: usize) -> Self {
        let mut q = VecDeque::with_capacity(capacity);
        for _ in 0..capacity {
            q.push_back(BytesMut::with_capacity(buf_size));
        }
        BufferPool { inner: Mutex::new(q), buf_size }
    }

    fn rent(&self) -> BytesMut {
        if let Some(mut b) = self.inner.lock().unwrap().pop_front() {
            b.clear();
            b
        } else {
            BytesMut::with_capacity(self.buf_size)
        }
    }

    fn give_back(&self, mut b: BytesMut) {
        b.clear();
        self.inner.lock().unwrap().push_back(b);
    }
}
```

- Use the pool in request handlers to avoid per-request Vec allocation.

- Allocator choices;
  - The default system allocator is fine, but for low-latency use `mimalloc` or `jemallocator` (mimalloc often shows lower tail latency).
  - To use mimalloc;
  ```toml
  # Cargo.toml
  [dependencies]
  mimalloc = { version = "0.1", features = ["release"] }

  # in main.rs
  #[global_allocator]
  static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
  ```
  - Test both in the Log Collector's workload; allocators' effects vary by workload.
- Avoid ephemeral allocations in hot paths;
  - Use stack-allocated small arrays where safe.
  - Use `smallvec` for small dynamic arrays that rarely exceed stack space.
  - Reuse objects (object pools) in high-throughput loops.
- Avoid `String`/`Vec` churn — prefer `Bytes` for slices and shared buffers.

---

### 8) Concurrency, Locking, and Async Patterns To Avoid
- Rules of thumb;
  - Never `.await()` while holding a lock.
  - Avoid `Mutex` in hot paths; use `parking_lot` for lower overhead if a lock is unavoidable.
  - Prefer lock-free structures: `ArcSwap`, `crossbeam` (SegQueue, MsQueue), `dashmap` for concurrent maps when reads dominate.
  - Use `RwLock` only where reads vastly outnumber writes and where write latency can be tolerated.

- Minimize `.await` points in tight loops. When writing loops that serve requests, avoid `await` and prefer synchronous operations until network I/O is necessary.

- Prefer message passing to shared state. Use `tokio::sync::mpsc` or `crossbeam-channel` with pre-allocated internal buffers to move work between threads rather than locking shared data.

---

### 9) CPU Affinity, NUMA, IRQ Pinning, and Thread Pinning
- Putting the right threads on the right cores matters a lot for tail latency.

- Core pinning;
  - Use `core_affinity` crate at runtime to set the current thread to a specific CPU core, for example;
  ```rust
  let cores = core_affinity::get_core_ids().unwrap();
  core_affinity::set_for_current(cores[2]); // pin this thread to core 2
  ```

- Pin;
  - Tokio worker threads handling network I/O to dedicated cores.
  - Polling threads (if any) to nearby cores.

- NUMA;
  - Make sure memory allocations and threads are NUMA-local. For multi-socket machines, keep threads and memory on the same NUMA node.
  - Use `numactl` in testing to confirm NUMA behavior.

- IRQ / NIC affinity;
  - Pin NIC interrupts to specific cores using `irqbalance` or `ethtool -L` and `echo` to `/proc/irq/.../smp_affinity`.
  - On cloud VMs, NIC-level control is limited; but on bare metal you must pin NIC IRQs.

- Disable SMT (hyperthreading) if jitter matters. In many low-latency workloads disabling SMT improves determinism. Test both options thoroughly.

---

### 10) Observability: Metrics, Tracing, and High-Precision Timing
- Ensure accurate measurements at nanosecond/microsecond resolution.

- Timing;
  - Use `std::time::Instant` for simple timing. For higher resolution timestamps, `quanta` or `time` crates with monotonic clocks are alternatives.
  - Always record start and end of the RPC handler, serialization, encoding, and network send/receive.

- Histogram library;
  - Use `hdrhistogram` for p50/p95/p99/p999 and to detect tail spikes.
  - Export histograms via metrics for automated CI checks.

- Tracking;
  - Use `tracing` crate with `tracing-subscriber` for structured logs.
  - Keep trace spans lightweight; avoid blocking operations in trace handlers.

- Tokio console;
  - Use `tokio-console` to inspect runtime tasks and blocking. It highlights hot tasks and excessive blocking.
  - Instrument example;
    ```rust
    use hdrhistogram::Histogram;
    let mut hist = Histogram::<u64>::new(3).unwrap(); // 3 significant digits
    let start = Instant::now();
    // do work
    let elapsed_us = start.elapsed().as_micros() as u64;
    hist.record(elapsed_us).unwrap();
    ```

- Expose metrics (Prometheus) with latencies and counters; set up an alert for p99 > threshold.

---

### 11) Benchmarking & Performance Testing Harnesses
- Conduct tests under realistic conditions; warm caches, warmed connections, representative payloads, and multi-threaded concurrency.

- A local micro-benchmark harness (async);
  - A simple client that sends N RPCs concurrently and measures latency:
    ```rust
    use tokio::time::Instant;
    use hdrhistogram::Histogram;

    async fn benchmark_client(client: MyGrpcClient<Channel>, concurrency: usize, total_requests: usize) {
        let mut hist = Histogram::<u64>::new(3).unwrap();
        let sem = tokio::sync::Semaphore::new(concurrency);

        let start_total = Instant::now();
        let mut handles = Vec::with_capacity(total_requests);
        for _ in 0..total_requests {
            let permit = sem.clone().acquire_owned().await.unwrap();
            let client = client.clone();
            handles.push(tokio::spawn(async move {
                let s = Instant::now();
                let _ = client.do_work(Request::new(MyRequest {})).await;
                let elapsed_us = s.elapsed().as_micros() as u64;
                drop(permit);
                elapsed_us
            }));
        }

        for h in handles {
            if let Ok(elapsed) = h.await.unwrap() {
                hist.record(elapsed).unwrap();
            }
        }

        println!("Total time: {:?}", start_total.elapsed());
        println!("p50: {}us p95:{}us p99:{}us p999:{}us",
            hist.value_at_percentile(50.0),
            hist.value_at_percentile(95.0),
            hist.value_at_percentile(99.0),
            hist.value_at_percentile(99.9));
    }
    ```

- Local testing;
  - Use `wrk`, `netperf`, `ghz` (for gRPC) to generate realistic traffic.
  - Ensure connections are re-used and warmed up; run warm-up phase (N seconds) before measurement.

- Test matrices;
  - Single client, single server (baseline)
  - Many clients (concurrent), single server
  - Multi-servce chains with internal RPCs (to measure cascade latency)
  - Long-run soak test (60-120 minutes) to reveal memory leaks and tail spikes.

- CI gating;
  - Automate a micro-benchmark in CI that runs for a short period and asserts that p99 < threshold. Be careful — CI environment noise is high; use dedicated performance CI runners when possible.

---

### 12) Profiling & Tail-latency Diagnosis
- When p99/p999 is bad, find out why.

- Tools;
  - `perf` + `flamegraph` (Linux) — CPU hotspots
  - `pprof` (if add pprof exporter) — Go-like profiles are possible
  - `eBPF` tools: `bpftools`, `bpftrace`, `opensnoop` to inspect syscalls, IO latencies
  - `tokio-console` to identify blocking tasks and pending futures
  - `tcpdump` / `wireshark` to inspec packet-level timing

- Steps;
  1. Reproduce spike with load generator.
  2. Capture CPU profile (`perf record -F 99 -p <pid> -g -- sleep 30`) then `perf report` or create a flamegraph.
  3. If syscall/blocking shows up, trace via `bpftrace` to see where syscalls are happening.
  4. Check GC-like behavior: allocation bursts, allocator stalls.
  5. Inspect NIC / OS-level queues: `netstat -s`, `ss -i` for re-transmissions or RTOs.
  6. Check interrupts and context switches: `vmstat`, `iostat`, `dstat`.

- Analyze tail events;
  - If spikes correlate with minor GC in external components (e.g., Python service nearby), adjust deployment or resource isolation.
  - If spikes occur on deployment/rollout events, spike due to cold-starts or JIT-like behaviors.

---

### 13) CI / Regression Testing for Latency Guarantees
- Testing in CI;
  - Use dedicated performance runners (bare metal, same instance type) — cloud CI is noisy.
  - Run a short warm-up (30-60s), then measurement for 60s.
  - Assert p50/p95/p99 numbers using `hdrhistogram`. Example; p99 < 1ms.

- Test artifacts;
  - Save histograms and flamegraphs as artifacts for each run.
  - Fail the build on regressions.

- Release gating;
  - Only promote builds that pass latency regression tests on a matching environment.

---

### 14) Common Pitfalls & Troubleshooting Checklist
- Holding locks across .await — top cause of jitter.
- Allocating per-request buffers — leads to heap churn and tail spikes. Use pools.
- Not warming connections — first request pays handshake cost. Warm connections in warm-up phase.
- Logging synchronously in hot loops — use batching, async logging, or drop debug logs in hot paths.
- Incorrect thread counts — oversubscription causes latency spikes. Match workers to physical cores.
- Using JSON for RPC — text formats are slow to parse.
- Not inspecting p999 — median looks fine but tails ruin UX. Monitor p999/p9999.
- Relying only on average — use histograms, not averages.

---

### 15) Advanced / Future Options (for VES Cloud)
- At this point we will control infrastructure and more options will be available to us;
  - Kernel bypass: DPDK, AF_XDP, or io_uring for even lower overhead. Use only for specific modules.
  - RDMA / InfiniBand / SmartNICs: For sub-100 µs messaging between nodes.
  - FPGA acceleration: Offload serialization/deserialization or pattern matching to hardware.
  - QUIC (HTTP/3): Gains from reduced head-of-line blocking and faster connection establishment for mobile clients.
  - Single-address space RPC: Shared memory or mmap for same-host communication.
  - CPU microcode / NIC tuning: Hugepages, CPU governor, NIC firmware settings.

- Refer to VES Cloud performance documentation when-ready for this step...

---

## Appendix A — Example "low-latency" Tonic server (complete)
```rust
// main.rs (abridged)
use tonic::transport::Server;
use tokio::net::TcpSocket;
use std::time::Duration;
use my_service::my_server::MyServer;
use my_impl::MyImpl;

#[tokio::main(flavor = "current_thread")] // or run from tuned runtime shown earlier
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure socket with TCP_NODELAY
    let socket = TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.set_nodelay(true)?;
    let addr = "0.0.0.0:50051".parse()?;
    let listener = socket.bind(addr)?.listen(1024);

    let svc = MyImpl::new();
    Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(5)))
        .http2_keepalive_timeout(Some(Duration::from_secs(5)))
        .initial_stream_window_size(Some(1024 * 1024))
        .initial_connection_window_size(Some(1024 * 1024))
        .add_service(MyServer::new(svc))
        .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
        .await?;

    Ok(())
}
```

- Uses a tuned runtime as discussed in Section 4 when you need separate thread control.
- Ensure `MyImpl` handler avoids allocations and `.await` inside locks.

---

## Appendix B — Roadmap for Rollout (practical steps)
1. Baseline: Run current benchmark, capture histograms, flamegraphs.
2. Build & flags: Enable `release` profile settings. Rerun benchmarks.
3. Buffers: Add buffer pool and switch serialization to `prost`. Measure.
4. Tokio runtime: Create dedicated runtime and pin it. Measure.
5. gRPC/tcp knobs: Enable TCP_NODELAY, increase HTTP/2 windows. Measure.
6. Allocators: Try `mimalloc` and compare histograms.
7. CPU pinning and NIC affinity: On a test node, pin workers + NIC IRQs. Measure.
8. Soak test: 60–120 minutes to detect leaks and jitter.
9. CI automation: Add microbench gating and artifact collection.
10. Iterate: Fix top hotspots from flamegraphs, repeat.

---

## Appendix C — Short Glossary / Cheat Sheet
- p50/p95/p99/p999 — percentiles of latency distribution. Look at p99/p999 for tail behavior.
- TCP_NODELAY — disables Nagle; send small packets immediately.
- LTO — Link Time Optimization.
- codegen-units = 1 — allows more cross-module optimization.
- Bytes / BytesMut — zero-copy buffer types used widely in async Rust.
- mimalloc / jemalloc — alternative allocators to reduce tail latency.
- DPDK / RDMA — advanced NIC/kernel bypass tech (future).

---

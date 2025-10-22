# Mission Overview
VES Log Collector is designed for high-throughput, low-latency log ingestion and shipping, optimized for modern compute environments. The goal is to outperform competitors like; Fluent Bit, Vector, and DataDog Agent in both raw performance and operational efficiency, providing a foundation for the full semantic search stack to be added later.

This document outlines:
- Performance and compute optimization targets
- Engineering principles
- Demo-readiness criteria and work plan

---

## Performance & Compute Optimization Targets
⚙️ Industry Baseline
```
| Metric           | Fluent Bit / Vector (Approx)     |
| ---------------- | -------------------------------- |
| Max Throughput   | 800K–1M log lines/sec (4-core)   |
| Median Latency   | 1–2 ms per log (ingest → ship)   |
| CPU Usage        | 30–50% of 1 core @ 100K logs/sec |
| Memory Footprint | 30–70 MB steady-state            |
```

VES must outperform by achieving:
- ~2x better CPU efficiency
- Lower p99 latency
- Smaller memory footprint and binary size

🧮 Target Performance Metrics
```
| Area                    | Metric                                            | Target                                          | Rationale |
| ----------------------- | ------------------------------------------------- | ----------------------------------------------- | --------- |
| **Throughput**          | ≥1.5M log lines/sec (local → batch → stdout/file) | Demonstrate superior hot-path efficiency        |           |
| **Latency (p99)**       | <1ms (local → batch handoff)                      | Feels instant to the user; critical for tailing |           |
| **CPU Efficiency**      | ≤25% of one core @ 100K logs/sec                  | 50% lower than Vector under identical load      |           |
| **Memory Footprint**    | ≤50 MB steady-state                               | Enables small container and edge deployments    |           |
| **Batch Flush Latency** | <10ms per 1K logs                                 | Verifies efficient batching pipeline            |           |
| **Startup Time**        | <100ms                                            | Instant start during CLI demos                  |           |
| **Binary Size**         | ≤10 MB                                            | Fast install, edge-friendly footprint           |           |
```

🧩 Compute Cost Target (Cloud Context)
If deployed in a SaaS or hybrid environment:
```
| Metric                     | Target            | Reference                 |
| -------------------------- | ----------------- | ------------------------- |
| Cost per 1M logs processed | ≤ **$0.10**       | (Vector ≈ $0.20–0.25)     |
| CPU time per log           | **50% reduction** | via Rust async + batching |
```

Formula:
CPU-seconds per 1M logs * instance $/CPU-second

🔬 Performance Engineering Breakdown
1. Async vs Multi-threading
- Use async I/O for tailing, parsing, and network shipping (non-blocking I/O, lower syscall overhead).
- Use dedicated threads for CPU-bound operations (compression, serialization, metrics aggregation).
- Offload expensive I/O or compute tasks via bounded worker pools.

2. Zero-Copy Abstractions
- Pass `&[u8]` slices between pipeline stages (no data cloning).
- Use ring buffers or mmap for staging batches.
- Use Bytes crate for immutable, shared memory segments.

3. Batching Strategy
- Tune batch size dynamically based on throughput (adaptive batching).
- Batch flush triggered by size or time threshold (whichever first).
- Typical defaults: `batch_size=1000`, `flush_interval=5ms`.

4. Compression
- Integrate zstd (C binding) for high-ration compression with minimal CPU.
- Use Rust wrappers (zstd-safe, zstd-sys).

5. Lock-Free Queues
- Employ SPSC ring buffers for stage communication (tailer -> batcher -> shipper).
- Consider `Crossbeam` or `tokio::sync::mpsc` with bounded capacity.

6. NUMA & Thread Tuning
- For multi-core systems: pin threads using `core_affinity`.
- Use per-thread allocator pools to minimize cross-core contention.

7. SIMD Optimization
- C can be introduced for tight numeric operations (e.g., compression, checksum).
- Use C intrinsics for SIMD acceleration if Rust performance plateaus.

🧰 Microbenchmark Suite
```
| Test                                 | Description                        | Metric                |
| ------------------------------------ | ---------------------------------- | --------------------- |
| **Tail → Parser → Batch throughput** | Feed 1GB file, measure lines/sec   | Throughput            |
| **Async vs Threaded pipeline**       | Compare concurrency models         | CPU usage, latency    |
| **Memory footprint**                 | `/usr/bin/time -v` or `psrecord`   | RSS (steady state)    |
| **Latency histogram**                | Emit timestamps before/after flush | p50, p95, p99 latency |
```

> Benchmark results will be shown in Grafana or terminal output during demo

---

## Demo-Ready Checklist
The goal is not a full product, but a minimum viable demonstration that feels production-ready. Ensure audience/users walk away convinced it already works.
```
| Area                | Goal                                                      | Done When                                                    |       |
| ------------------- | --------------------------------------------------------- | ------------------------------------------------------------ | ----- |
| **Core Pipeline**   | Full chain: tail → parse → filter → enrich → batch → ship | `ves-log-collector --demo` shows logs processed in real time |       |
| **Config System**   | Readable YAML/TOML config                                 | User sets `watch_path`, `batch_size`, `ship_url`, etc.       |       |
| **CLI UX**          | Polished binary with help output                          | `ves-log-collector --help` shows formatted options           |       |
| **Metrics & Stats** | Local `/metrics` or `--stats` flag                        | Prints throughput, CPU %, memory, latency                    |       |
| **Installer**       | Prebuilt binary or one-liner install                      | `curl -fsSL install.ves.sh                                   | bash` |
| **Benchmark Mode**  | CLI benchmark tool                                        | `ves-log-collector bench --duration 10s` prints live stats   |       |
| **Optional UI**     | Web dashboard via WebSocket/SSE                           | Streams live stats (nice-to-have)                            |       |
| **Docs**            | README with quick start                                   | “How to run in 3 commands”                                   |       |
| **Demo Script**     | Rehearsed 2-min flow                                      | Works offline, reproducible output                           |       |
```

---

## Final Week Prep Plan
```
| Day     | Focus                                                    |
| ------- | -------------------------------------------------------- |
| **1–2** | Finalize config + CLI UX + `/metrics` endpoint           |
| **3–4** | Optimize batching and implement microbench harness       |
| **5**   | Implement `--demo` mode (ship to stdout + live stats)    |
| **6**   | Add `--bench` mode for synthetic load tests              |
| **7**   | Run real benchmarks, produce throughput & latency graphs |
| **8**   | Polish README, write install script, and prepare binary  |
| **9**   | Rehearse live demo + record fallback video               |
| **10**  | Final QA & test in event environment                     |
```

---

## MVP Demo Execution Plan
🎬 CLI Demo Script
Terminal 1 - Run the collector
```console
ves-log-collector --demo --watch ./nginx.log
```

Terminal 2 - Generate logs
```console
tail -f nginx.log >> ./nginx.log
```

Expected Output (live updates)
```console
Throughput: 1.42M logs/sec | CPU: 23% | Latency p99: 0.6ms | Memory: 48MB
```

Talking Points:
> "VES Log Collector is written in Rust for high-throughput, low-latency ingestion. It processes over 1.4M logs/sec while using less than 50MB of memory. That's about twice as efficient as most of what is on the market."

📊 Benchmark Demo
```console
ves-log-collector bench --duration 10s --batch-size 1000
```

Output:
```toml
[Benchmark Results]
Throughput: 1.58M logs/sec
Latency p95: 0.8ms
CPU: 24%
Memory: 46MB
```

Graph results (for slides):
- Latency percentile plot (p50, p95, p99)
- Throughput vs CPU usage chart

---

## Summary
```
| Goal                       | Target                                                   |
| -------------------------- | -------------------------------------------------------- |
| **Performance Leadership** | 2× throughput & ½ compute cost of incumbents             |
| **Demo-Readiness**         | Complete end-to-end ingestion pipeline with live metrics |
| **User Impression**        | “It’s already production-grade.”                         |
| **Next Step**              | Add embeddings + indexing service                        |
```

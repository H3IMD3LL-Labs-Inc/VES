# Log Collector(DaemonSet)

The log-collector microservice is intended to run as a DaemonSet(one pod per node). It tails container log files on that node, reconstructs events (incl. multi-line stack traces), enriches with k8s metadata, applies filters/redaction, then ships batches to the Embedding micro-service over the network, can be HTTP or gRPC.

## Components

1. **Watcher**
  - Watches container log files from where they are stored on the node the log collector is on, e.g(/var/log/containers/*.log).
  - Detects when; **A new pod/container starts -> new log file appears**, **Log rotation happens -> old file renamed, new one created.
  - Keeps track of where it left off (offsets) so it doesn't re-read old logs.

2. **Tailer**
  - Continuously reads new lines as they're written to the log files
  - Understands Kubernetes/CRI log format (which includes timestamps, stdout/stderr, and "partial" markers).
  - Reconstructs multi-line logs (e.g., Python/Java stach traces) into one event.

 👉 This is basically where raw log text becomes structured "messages".

3. **Parser**
  - Detects whether the log line is:
    - JSON (structured logs from apps) -> parse into fields.
    - Plaintext -> keeps the raw string.
  - Extracts a log level if possible, e.g., `INFO`, `WARN`, `ERROR`.

 👉 This adds structure so later stages can filter, enrich, and search intelligently.

4. **Metadata Enricher**
  - Attaches context to logs from Kubernetes:
    - `namespace`, `pod`, `container`, etc.
    - Labels/annotations, e.g., `app=payment-api`, `env=prod`, etc.
    - Node info, e.g., `node_name`, `host_ip`, etc.
  This is intended to make logs searchable not just by text but also by where they came from.

 👉 Without this, all logs are just random text lines - metadata is intended to make this tool actually useful.

5. **Filter & Redactor**
  - Lets you configure rules such as:
    - Ignore logs from `kube-system` namespace.
    - Drop `DEBUG` logs if too noisy.
    - Redact sensitive data (e.g., API_KEYS, emails, etc.)
  - Optional sampling, e.g., `only keep 10% of DEBUG logs` can also be setup using this component.

 👉 This is just to keep the system efficient and protect against sensitive data leaks.

6. **Buffer & Batcher**
  - Collects `events` into batches, e.g., `200 logs or every 500ms`.
  - Optionally store batches in a local buffer/WAL (write-ahead log) so they aren't lost incase of pod crashes, etc.
  - Compresses batches(`gzip`) before sending them to embedding service via the `Shipper` component.

 👉 This is just to ensure efficiency and reliability under load.

7. **Shipper**
  - Sends log batches to the **Embedding Service** via HTTP or gRPC.
  - Handles retries if the embedding service is down/unavailable.
  - Implements at-least-once delivery (Using IDs or offsets to avoid log duplication).
  - Deals with backpressure, e.g., `slowing down or dropping low-priority logs if the embedding service is overloaded` (This is just a safety measure because the Embedding Service should be highly available at all times).

 👉 This is basically the output pipe for the Log-Collector microservice DaemonSet that will be running in a node.

8. **Control & Observability**
  - Config Loader -> reads settings from a ConfigMap or env vars.
  - Metrics endpoint -> exposes prometheus stats like logs read, logs dropped, retries.
  - Health Checks -> these are to ensure Kubernetes knows the micro-service is working/up/available.
  - Self-logs -> This is to allow users to debug the collector itself.

 👉 This should be fully reliant on the log collector, but for now integration with prometheus is used to move the project forward, fast.

Built with ❤️ for A1m4 N4d1n3 a.k.a miss ma'am :)

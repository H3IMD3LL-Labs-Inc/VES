# Vector-Enhanced Log & Metric Search

This is a Kubernetes-native(by design) semantic log search layer that can plug into existing Loki/ELK setups, powered by embeddings and ChromaDB, with an optional standalone mode.

Traditional log search (ELK, Loki, Splunk) is **keyword-based** - you only find exact matches. But logs often express the same error many different ways:

- `"NullPointerException"`
- `"object reference not set"`
- `"NoneType error"`

By embedding logs into **vector spaces** with an embedding model, we can search **semantically**:
> "Find logs like this one" instead of exact keyword matching.

This repo builds a basic MVP of what a truly **semantic log search platfrom** using **Kubernetes, Loki/ELK, ChromaDB, and embeddings**.

---

## Features

- Semantic log search across services
- "Find similar logs" by pasting a stack trace
- Cluster logs by similarity -> spot recurrings issues faster
- Enrich alerts with past related logs
- Runs on Kubernetes, integrates with existing Loki/ELK stacks

---

## System Architecture

```
┌──────────────┐
│ Applications │
└───────┬──────┘
        │
 ┌──────▼────────┐
 │ Log Collector │
 └──────┬────────┘
        │
┌───────▼────────┐
│  Log Storage   │
└───────┬────────┘
        │
┌───────▼────────┐
│ Embedding Svc  │
└───────┬────────┘
        │
┌───────▼────────┐
│   ChromaDB     │
└────────────────┘

```

---

## Components

1. **Log Collector**
    - Has 2 modes: **bootstrapped(default)** and **automated(for new users)**
      1. Bootstrapped(default) mode assumes a user/team already runs Promtail/FluentBit -> Loki/ELK.
      2. Automated(for new users) mode assumes a user/team doesn't have Loki/ELK setup, this is shipped with a Promtail/FluentBit DaemonSet YAML that sends logs directly to the Embedding Service. This does not require Loki.
    - Uses promtrail or FLuentBit as DaemonSets in Kubernetes.
    - Collect logs from app pods.
    - Forward logs both to **Loki/ELK** and to the embedding sidecar.

2. **Embedding Sidecar**
    - A small micro-service that accepts raw logs and converts log lines into embeddings.
    - Uses a transformer, i.e, `sentence-transformers/all-MiniLM-L6-v2`, for logs to embeddings conversions.
    - Packages log + metadata + embedding into a document.
    - Pushes to ChromaDB.

    Example metadata:
    ```json
    {
        "id": "log123",
        "embedding":[...],
        "metadata": {
            "service": "payment-api",
            "timestamp": "2025-08-28T12:00:00Z",
            "log_level": "ERROR",
            "raw_log": "NullPointerException at line 42"
        }
    }

3. **ChromaDB Service**
    - Acts as the Vector database where log vector embeddings are indexed and stored.
    - Runs as a StatefulSet in Kubernetes, providing persistent storage.
    - Stores vector embeddings of logs.

4. **Search API Service**
    - Simple API endpoint(s) to; accept query log text, convert to embedding, query ChromaDB(through chromadb service), etc.
    - Results include:
        - Raw log text.
        - Service, timestamp, severity.
        - (if Loki/ELK present): link back to Grafana/Kibana with log context.

5. **(Optional) Web UI**
    - Minimalistic React app UI where Engineers can:
        - Paste logs or stack traces.
        - View "similar logs" instantly.
        - Click a result to jump into Loki/Grafana/Kibana/etc (if integrated).

---

## Project Modes

1. **Bootstrapped**: Assumes a user/team has Loki/ELK already setup and in use. Please read our instructions for tapping into your logs(Coming Soon).

2. **Automated**: Assumes user/team does not have Loki/ELK set up, includes an already provisioned Promtail(or whatever you'd want to use) DaemonSet YAML -> Logs -> Embedding Service -> ChromaDB.

---

## Contributing

Pull requests welcome - this is a hackable infra experiment, I honestly don't expect it to be big lol :-). My long-term goal is to basically have a production-ready semantic log search for DevOps teams.

---

Built with ❤️ for A1m4 N4d1n3 a.k.a miss ma'am :)

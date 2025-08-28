# Vector-Enhanced Log & Metric Search (with Kubernetes + ChromaDB)

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
 │ Log Collector │  (FluentBit / Promtail)
 └──────┬────────┘
        │
┌───────▼────────┐
│  Log Storage   │  (Loki / ELK)
└───────┬────────┘
        │ (sidecar / stream)
┌───────▼────────┐
│ Embedding Svc  │  (MiniLM, BERT, CLIP)
└───────┬────────┘
        │
┌───────▼────────┐
│   ChromaDB     │  (vector index)
└────────────────┘

```

---

## Components

1. **Log Collector**
    - Uses promtrail or FLuentBit as DaemonSets in Kubernetes.
    - Collect logs from app pods.
    - Forward logs both to **Loki/ELK** and to the embedding sidecar.

2. **Embedding Sidecar**
    - A small service that converts log lines into embeddings.
    - Uses `sentence-transformers/all-MiniLM-L6-v2` (for now/testing).
    - Pushes embeddings + metadata to ChromaDB.

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

3. **ChromaDB**
    - Stores vector embeddings of logs.
    - Runs as a StatefulSet in Kubernetes.

4. **Search API**
    - Accepts queries (log text, stack traces).
    - Converts to embedding.
    - Queries ChromaDB for top-K similar logs.
    - Returns logs + metadata + optional link back to ELK/Loki UI.

5. **(Optional) Web UI**
    - React dashboard for semantic search.
    - Paste logs -> see similar results instantly.

---

## Contributing

Pull requests welcome - this is a hackable infra experiment, I honestly don't expect it to be big lol :-). My long-term goal is to basically have a production-ready semantic log search for DevOps teams.

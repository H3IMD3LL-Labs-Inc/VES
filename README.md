# VES - Vector Enhanced Search (for logs)

⚠️ Status: Pre-MVP (only log file watcher prototype exists).
Expect breaking changes, incomplete features, and rapid iteration.

> Kubernetes-native semantic log search for modern DevOps teams.
> Plug into existing log collection setups or run standalone, powered by an embedding sidecar + ChromaDB.

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-green.svg)](./LICENSE)
[![Docs](https://img.shields.io/badge/docs-coming%20soon-blue)]()

---

## Why VES?

Traditional log search (ELK, Loki, Splunk, etc.) is **keyword-based** - you only find exact matches. But logs often express the same issue in many different ways:

- `"NullPointerException"`
- `"object reference not set"`
- `"NoneType error"`

By embedding logs into **vector space**, VES enables **semantic search**:
*"Find logs like this one"* instead of rigid keyword matching.

---

## 🚀 Features
- 🔍 Semantic log search across services
- 📋 “Find similar logs” by pasting a stack trace
- 🧩 Cluster logs by similarity → spot recurring issues faster
- ⚡ Enrich alerts with past related logs
- 🛠️ Kubernetes-native, integrates with existing Loki/ELK stacks
- 🧑‍💻 Optional standalone mode (no Loki required)

---

## 🏗️ System Architecture

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
│ Log Storage    │
└───────┬────────┘
        │
┌───────▼────────┐
│ Embedding Svc  │
└───────┬────────┘
        │
┌───────▼────────┐
│ ChromaDB       │
└────────────────┘
```
---

## 🔧 Components

1. **Log Collector**
   - Two modes:
     - **Bootstrapped (default)** → assumes a log-collection tool are already running
     - **Automated** → ships with log-collector microservice DaemonSet YAML for direct embedding ingestion
   - Runs as DaemonSets in Kubernetes
   - Forwards logs to both **Loki/ELK** and the **Embedding Sidecar**

2. **Embedding Sidecar**
   - Converts raw logs into embeddings (e.g. `sentence-transformers/all-MiniLM-L6-v2`)
   - Packages: log + metadata + embedding → sends to ChromaDB

   Example metadata:
   ```json
   {
       "id": "log123",
       "embedding": [...],
       "metadata": {
           "service": "payment-api",
           "timestamp": "2025-08-28T12:00:00Z",
           "log_level": "ERROR",
           "raw_log": "NullPointerException at line 42"
       }
   }
   ```

3. **ChromaDB Service**
   - Vector database for log embedding storage
   - Runs as a StatefulSet in Kubernetes with persistent storage

4. **API Service**
   - Accepts log queries, converts to embedding, searched ChromaDB
   - Returns: raw log, timestamp, service, severity
   - (Coming Soon): If log-collector running in bootstrapped mode, deep-links back into the bootstrapped log-collector

5. **Web UI (Coming Soon)**
   - Minimal React frontend
   - Paste logs / stack traces -> get "similar logs"
   - Click through to bootstrapped log-collector UI

---

## 📦 Project Modes
1. Bootstrapped -> for teams already using a log-collector
2. Automated -> for teams without any log infrastructure(RECOMMENDED)

---

## 🚀 Getting Started
```
# clone the repo
git clone https://github.com/H3IMD3LL-Labs-Inc/VES-Vector-Enhanced-Search-.git
cd VES-Vector-Enhanced-Search-.git

# (Coming Soon) Install via Helm or kubectl
kubectl apply -f deploy/ves.yaml
```

---

## 🤝 Contributing
Contributors are welcome!
Please see our upcoming [CONTRIBUTING](./CONTRIBUTING.md) and [CODE_OF_CONDUCT](./CODE_OF_CONDUCT.md).

This is currently a v0.1 hackable infra experiment — long-term goal(afew months): ➡️ production-ready semantic log search for DevOps teams.

---

## 📜 License
This project is licensed under the [Business Source License 1.1](./LICENSE).
- Licensor: H3IMD3LL Labs, Inc.
- Change Date: 5 years from publication
- Change License: MPL 2.0

---

## 📬 Community & Support
- [Email](mailto:dennis.njuguna@heimdelllabs.cloud)
- [Website](https://heimdelllabs.cloud/)
- [Discord(Coming Soon)]()

---

## 📍 Roadmap
- [x] v0.0.0 - Pre-MVP: dev logs, README, roadmap, discussions.

- [ ] v0.0.1 - MVP: log-collector, embedding-service, API-Service, Web UI
    - Log Collector
      ![25%](https://progress-bar.xyz/25/?style=minimal-matte)
    - Embedding Service
      ![0%](https://progress-bar.xyz/0/?style=minimal-matte)
    - API Service
      ![0%](https://progress-bar.xyz/0/?style=minimal-matte)

- [ ] v0.1.0 - Improvements: Documentation and Stability
    - Documentation
      ![25%](https://progress-bar.xyz/25/?style=minimal-matte)
    - Stability
      ![0%](https://progress-bar.xyz/0/?style=minimal-matte)

- [ ] v1.0.0 - Production-ready release: Further Documentation, Stability improvements and User Support
    - Further Documentation
      ![0%](https://progress-bar.xyz/0/?style=minimal-matte)
    - Stability improvements
      ![0%](https://progress-bar.xyz/0/?style=minimal-matte)
    - User Support
      ![0%](https://progress-bar.xyz/0/?style=minimal-matte)

---

Built with ❤️ for A1m4 N4d1n3 a.k.a miss ma'am :)

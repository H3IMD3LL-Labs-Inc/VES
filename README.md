<p align="center">
    <!-- Add VES logo here -->
</p>
<p align="center">
    <!-- Codecov -->
    <a href="https://codecov.io/gh/H3IMD3LL-Labs-Inc/VES"><img alt="Code Coverage" src="https://codecov.io/gh/H3IMD3LL-Labs-Inc/VES/branch/main/graph/badge.svg?token=YLBMDY8WC0"/></a>
    <!-- LOC -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES"><img alt="LOC" src="https://img.shields.io/endpoint?url=https://ghloc.vercel.app/api/H3IMD3LL-Labs-Inc/VES/badge?filter=.rs$,.sh$,.toml$&amp;style=flat&amp;logoColor=white&amp;label=Lines%20of%20Code" /></a>
    <!-- Contributors -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES/graphs/contributors"><img alt="GitHub contributors" src="https://img.shields.io/github/contributors/H3IMD3LL-Labs-Inc/VES?style=flat"/></a>
    <!-- PRs welcome -->
    <a href="https://makepullrequest.com"><img alt="PRs Welcome" src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=flat"/></a>
    <!-- Releases -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES/releases"><img alt="GitHub releases" src="https://img.shields.io/github/v/release/H3IMD3LL-Labs-Inc/VES?style=flat"/></a>
    <!-- Commit activity -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES/commits/main"><img alt="GitHub commit activity" src="https://img.shields.io/github/commit-activity/w/H3IMD3LL-Labs-Inc/VES?style=flat"/></a>
    <!-- Issues -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/H3IMD3LL-Labs-Inc/VES?style=flat&cacheSeconds=60"/></a>
    <!-- Stars -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES/stargazers"><img alt="GitHub stars" src="https://img.shields.io/github/stars/H3IMD3LL-Labs-Inc/VES?style=flat&cacheSeconds=60"/></a>
    <!-- Forks -->
    <a href="https://github.com/H3IMD3LL-Labs-Inc/VES/forks"><img alt="GitHub forks" src="https://img.shields.io/github/forks/H3IMD3LL-Labs-Inc/VES?style=flat&cacheSeconds=60"/></a>
    <!-- Docker stats (pulls & stars) (use correct repo name when ready) -->
    <!--
    <a href="https://hub.docker.com/r/heimdelllabs/ves">
        <img alt="Docker Pulls" src="https://img.shields.io/docker/pulls/heimdelllabs/ves?style=flat"/>
    </a>
    <a href="https://hub.docker.com/r/heimdelllabs/ves">
        <img alt="Docker Stars" src="https://img.shields.io/docker/stars/heimdelllabs/ves?style=flat"/>
    </a>
    -->
</p>

<p align="center">
    <a href="https://ves.heimdelllabs.com/docs">Docs</a> - <a href="https://discord.gg/">Discord</a> - <a href="https://x.com/heimdell_labs">X/Twitter</a> - <a href="https://ves.heimdelllabs.cloud/roadmap">Roadmap</a> - <a href="https://ves.heimdelllabs.cloud/why">Why VES?<a/> - <a href="https://ves.heimdelllabs.cloud/changelog">Changelog</a>
</p>

<p align="center">
    <!-- Add VES demo video here -->
</p>

## VES is a high performance, highly configurable and easy-to-understand observability data agent-aggregator

[VES](https://ves.heimdelllabs.cloud/) is an easy to understand, high performance log collection and observability agent-aggregator that makes it easy to add observability to your software at any scale. VES primarily focuses on three core aspects: high performance, ease of use/understanding and configurability.

You no longer need to read hundreds of docs from different tools just to add observability and log collection to your stack with almost undifferentiated performance and ever increasing complexity the deeper you integrate.

## Table of Contents

- [VES is a high performance, highly configurable and easy-to-understand observability data agent-aggregator](#ves-is-a-high-performance-highly-configurable-and-easy-to-understand-observability-data-agent-aggregator)
- [Guiding principles](#guiding-principles)
- [Use cases](#use-cases)
- [Comparisons](#comparisons)
- [Getting started with VES](#getting-started-with-ves)
  - [Self-hosting the open-source Beta](#self-hosting-the-open-source-beta)
  - [VES Cloud (Coming Soon)](#ves-cloud-coming-soon)
- [Setting up VES](#setting-up-ves)
- [Learning more about VES](#learning-more-about-ves)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)


## Getting started with VES

### Self-hosting the open-source Beta

Currently, the only way to use the latest development version of VES is self-hosting on linux. Their are no pre-built binaries or docker images for the current latest version of VES. We're working on adding these to get you started with VES faster.

See [Building VES](building.md)

### VES Cloud (Coming Soon)

The fastest and easiest way to get started with VES will be signing up for free to VES Cloud.

See [VES Cloud(Coming Soon)](https://ves.heimdelllabs.cloud/signup)

## Setting up VES

Before you've built a VES binary containing the latest development of VES, [Build VES](building.md). Configure various setting in the [configuration file](services/log-collector/src/config) to your preferences.

> Currently, VES does not support on-the-fly configuration settings. Work is being done to implement this by v1.0.0

Example configuration file:
```toml
[general]
enable_local_mode = true                                        # whether VES is watching log files locally (on the same node as logs are produced)
enable_network_mode = true                                      # whether VES is watching log files over a network (logs are produced on a different node)

[watcher]
enabled = true
checkpoint_path = "path/to/checkpoint.json"                     # path to the VES checkpoint file, required to support resuming on crashes or restarts
log_dir = "path/to/log/file(s)/directory"                       # path VES will use to watch and tail log files it's working on local mode
poll_interval_ms = 5000                                         # poll interval local log file watcher will use to check for any updates to the log_dir
recursive = false                                               # whether the local log watcher will watch files in log_dir recursively

[parser]
# Currently, the parser module is non-configurable, work is being done to implement this

[buffer]
capacity_option = "unbounded"                                   # Options: "bounded", "unbounded" determines what InMemoryBuffer capacity is set to at runtime
buffer_capacity = 10000                                         # determines the capacity InMemoryBuffer will be created with at runtime
batch_size = 200                                                # determine InMemoryBuffer batch size at runtime
batch_timeout_ms = 500                                          # determine InMemoryBuffer batch size based on the size at a point in time at runtime
overflow_policy = "drop_oldest"                                 # determine how to handle InMemoryBuffer overflow at runtime
flush_policy = "batch_size"                                     # determine InMemoryBuffer flush trigger at runtime
drain_policy = "batch_size"                                     # determine InMemoryBuffer draining trigger at runtime

[buffer.durability]
type = "s-q-lite"                                               # determine the durability settings for InMemoryBuffer at runtime. RECOMMENDED s-q-lite to allow graceful restarts without losing data during a crash
path = "/var/ves-sqlite-db/parsed_log_buffer.db"                # determine location of the SQLite DB providing persistence to InMemoryBuffer at runtime

[shipper]
embedder_target_addr = "https://127.0.0.1:50051"                # determine the gRPC address for the Embedding engine. Required to send aggregated/normalized logs to be converted to vector embeddings
connection_timeout_ms = 500                                     # determine time to wait before failing a new connection attempt to the Embedding engine
max_reconnect_attempts = 10                                     # determine reconnection to Embedding engine limit before declaring reconnection failure
initial_retry_delay_ms = 500                                    # determine delay before reconnection retry attempts to Embedding engine
max_retry_delay_ms = 30000                                      # determine ceiling for exponential backoff while retrying reconnections to Embedding engine
backoff_factor = 2.0                                            # determine multiplier for reconnection to Embedding engine retry growth
retry_jitter = 0.2                                              # determine random reconnection to Embedding engine jitter percentage to avoid thundering herd problem on successful reconnection to Embedding engine
send_timeout_ms = 3000                                          # determine max timeout to push one aggregated/normalized logs batch into Embedding engine gRPC stream
response_timeout_ms = 10000                                     # determine max timeout to wait for response from Embedding engine before declaring gRPC stream unhealthy
```

The above example configuration file can be used as a base to start using VES.

>It is recommended to configure the configuration file before compiling a VES binary. VES currently does not support on-the-fly configuration.

## Learning more about VES

Curious about how to make the most of VES? See our [docs](https://ves.heimdelllabs.cloud/docs) for anything that you think is not in the README.

## Roadmap

VES is current roadmap is at getting the current `v0.1.0` Beta version to `v1.0.0` Stable version. The roadmap is whatever is currently opened in [Issues](https://github.com/H3IMD3LL-Labs-Inc/VES/issues).

The goal is to release a stable version `v1.0.0` prior to the new year.

## Contributing

All contributions make VES better and help achieve the goals set out for the project <3.

See [How can I contribute](contributing.md)

## LICENSE

VES is free and the source is available. Currently, VES is released under the AGPL license. See individual files for details which will specify the license applicable to each file.

For more information see the [VES License](license.md)

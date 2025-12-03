# Building VES

Please note that VES **DOES NOT** currently have any pre-built binaries or Docker images. Work is currently being done to make this the easiest way to get started, rather than building from source.

To build VES, you will need:

- [Rust](https://rust-lang.org/tools/install/)
  - It is recommended to use this toolchain version `rustc 1.89.0 29483883 2025-08-0` and this toolchain manager version `rustup 1.28.2 e4f3ad6f8 2025-04-28`. Other versions may work, but at the time of initial development this is what was used.

- [Protocol Buffers](https://protobuf.dev/installation/)
  - It is recommended to use this protocol buffers compiler version `libprotoc 30.2`. Other versions may work, but at the time of initial development this is what was used.

- About 30GB of free disk space for core agent-aggregator binary package `ves`.

VES currently supports the following PC architectures: x86_64

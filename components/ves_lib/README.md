# Overview
`ves_lib` is a *shared foundation layer* and the *backbone of inter-component communication* for the VES Platform.

It exists, currently, to provide a single source of truth for:
- Data contracts (Protobuf -> Rust structs)
- Shared logic used across components
- Communication primitives for the custom transport(future implementation)

Ensuring:
- Consistent data contracts across `/components`
- Clean separation between component logic and data transport
- Flexibility to evolve communication mechanisms
- Strong foundation for a platform-level testing suite

It is intentionally transport-agnostic and framework-independent, allowing for some neat tricks and edge cases while starting out simply and becoming increasingly critical as VES grows into a broader platform.

`ves_lib` is a library for the VES platform, not a system component ;)

---

## Purpose
[VES](https://github.com/H3IMD3LL-Labs-Inc/VES) is designed as a multi-component platform, where independent binaries (e.g., `core-agent`, `heimdell`) communicate using structured data.

To avoid:
- Duplicate message definitions
- Unnecessarily tight [/components](https://github.com/H3IMD3LL-Labs-Inc/VES/tree/v1.0.0-unstable/components) coupling
- Inconsistent serialization logic across `/components`

`ves_lib` centralizes all these shared concerns away from individual components' binaries

---

## Single Responsibilities
1. *Data Contracts (Primary Responsibility)*
    - Contains Rust structs generated from `.proto` files from `/proto` via [Buf's](https://buf.build/docs/) `buf generate`
    - Defines all shared messages' formats, i.e, `ConfigEvent`, `RootConfig`, `EnrichedEvent`, etc.
This ensures type safety across /components, schema consistency and no duplication of message definitions.

2. *Shared Utilities (Optional, Growing)*
    - Validation helpers
    - Common data transformations
    - Cross-component logic that does not belong to a single /components' binary

3. *Future Custom Transport Layer (Optional)*
`ves_lib/transport` will eventually contain a *custom networking layer*, if Tonic/gRPC stops being useful for inter-component communication. This layer and logic will replace any current gRPC-based communication VES is using, time will tell though

---

## How Components Use `ves_lib`
[/components/core-agent](https://github.com/H3IMD3LL-Labs-Inc/VES/tree/v1.0.0-unstable/components/core-agent) uses the shared structs in `ves_lib` to construct events and send them to `/heimdell` via the current transport *Tonic/gRPC*

[/components/heimdell](https://github.com/H3IMD3LL-Labs-Inc/VES/tree/v1.0.0-unstable/components/heimdell) receives and processes the events from a Core Agent

The above is because both components share the same types, but not the same transport implementation. Ensuring data consistency and flexible inter-service communication

---

## Inter-Component Communication Model (v1)
Currently, `core-agent` and `heimdell` communicate via gRPC/Tonic, which uses the schemas defined in [/proto](https://github.com/H3IMD3LL-Labs-Inc/VES/tree/v1.0.0-unstable/proto). The Tonic/gRPC implementations using these schemas is present/used only inside the /components' binaries, `ves_lib` is not aware of gRPC implementations

This way, data contracts are independent of transport, which is independent of application logic and all shared types live in one place inside `/ves_lib/codegen` preventing logic drift between components.

The communication mechanism can change without affecting any important logic, enabling migration from gRPC/Tonic -> Custom Protocol

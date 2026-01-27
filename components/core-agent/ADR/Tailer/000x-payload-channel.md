# ADR: TailerManager Channel ownership and TailerPayload Flow

## Status: *Accepted*

## Context
In the tailing system, we have multiple Tailer tasks being spawned when `start_tailer()` is called. Each Tailer task is responsible for doing what a Tailer is designed to do (read data from a file, create a TailerPayload and send the TailerPayload downstream). To do this the following requirements are met:

- Each Tailer must independently emit its TailerPayload concurrently.
- There must be a single consumer stage downstream that processes each Tailer's TailerPayloads.
- Backpressure and ordering must be preserved.
- Each Tailer's lifecycle must be managed cleanly and ONLY BY TailerManager.

Initially, I was planning on going with a naive design where each Tailer would create its own independent channel, this albeit being easier and more straightforward to code would lead to:

- Producing N independent queues with multiple receivers
- Lose global ordering across each independent Tailer and its channel
- Complicate backpressure and shutdown handling 

Overall, I realised I hate dealing with tech debt later on lol :)....

## Decisions
- *Channel Creation:* The TailerManager creates a single `mpsc::channel::<TailerPayload>(1024)` to serve all Tailers that will be spawned at runtime.
- *Tailer Ownership:* Each Tailer receives a cloned `mpsc::Sender<TailerPayload>` from the manager, allowing each Tailer to send its TailerPayloads independently.
- *Manager Responsibility:* TailerManager's core responsibility, Tailer lifecycle orchestration (start, stop, cancellation), still remains. IT DOES NOT SEND TailerPayloads. This responsibility still belongs to each independent Tailer.
- *CancellationTokens:* Each spawned Tailer has a child CancellationToken from the TailerManager's CancellationToken for clean shutdown

This design ensures:
- A single source of truth for the channel
- Clear separation of responsibilities in the TailerManager and Tailers
- Concurrent Tailer tasks sending TailerPayloads safely
- Backpressure handled automatically by the mpsc queue
- Unified downstream consumer with ordered payloads

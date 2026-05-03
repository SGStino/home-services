# Architecture

## Principles

- The system is implemented in Rust.
- The platform uses a distributed microservice model.
- A central message bus is the backbone for event-driven communication and state synchronization.
- Each device is handled by exactly one microservice instance.
- Each device microservice must have a minimal resource footprint.
- Device protocol handling must be isolated from event bus integration concerns.

## Code organization guardrails

To keep service implementations maintainable as features grow, each crate should keep
transport, mapping, runtime orchestration, and protocol-specific concerns in separate modules.

Current baseline module boundaries:

- `hs-eventbus-mqtt-ha`
	- `adapter.rs`: adapter-facing API and trait implementation entrypoints
	- `config.rs`: Home Assistant MQTT config defaults and environment-based config assembly (service/node scoped, not tied to one device descriptor)
	- `transport.rs`: MQTT client options and event-loop management
	- `command.rs`: command route typing and command payload mapping helpers
	- `payloads.rs` and `topics.rs`: Home Assistant payload and topic conventions
- `hs-core`
	- `runtime.rs`: device runtime orchestration and publish paths
	- `device_service.rs`: reusable service lifecycle loop (startup, command/tick loop, shutdown)
	- `runtime_metrics.rs`: runtime metric instruments and initialization
	- `state_filter.rs`: shared, device-agnostic state deduplication utility with optional numeric delta-threshold filtering and max-silence periodic re-emit; profiles are configured by each device service.
	- `telemetry.rs`: OpenTelemetry and tracing initialization
- `hs-service-device-demo`
	- `main.rs`: process entrypoint only
	- `app.rs`: demo behavior implementation (tick + command handling) wired to shared lifecycle runner
	- `bootstrap.rs`: demo device and capability assembly only
	- `command_payload.rs` and `time.rs`: small focused helpers
- `hs-service-device-esphome`
	- `main.rs`: process entrypoint only
	- `app.rs`: ESPHome bridge behavior implementation (state drain + command forward) wired to shared lifecycle runner
	- `config.rs`: environment-driven device metadata plus ESPHome API endpoint and auth settings
	- `esphome.rs`: ESPHome native API client, entity discovery, state decoding, and command forwarding
	- `time.rs`: timestamp helper
- `hs-service-device-tplink-hs110`
	- `main.rs`: process entrypoint only
	- `app.rs`: HS110 behavior implementation (poll + command handling) wired to shared lifecycle runner
	- `config.rs`: environment-driven device metadata plus HS110 host/port/timeout settings
	- `hs110_client.rs`: TP-Link HS110 request/response operations and snapshot modeling
	- `tplink_protocol.rs`: TP-Link local framing and XOR cipher helpers
	- `bootstrap.rs`, `command_payload.rs`, and `time.rs`: focused capability, command parsing, and timestamp helpers

Guideline:

- avoid combining connection management, event loops, payload translation, and
	business-level command handling in a single file
- prefer adding a new module once a file starts carrying more than one primary
	responsibility

## Architectural model

The platform is composed of many small device-focused microservices plus a shared message bus.

Each device microservice is a three-tier application:

1. Device communication
2. Central core
3. Event bus adapter

The three tiers have distinct responsibilities.

### Device communication

This tier speaks the native device protocol and deals with the device directly.

Examples:

- raw MQTT topics for a specific device
- vendor HTTP APIs
- BLE or serial communication
- fieldbus or industrial protocols

Responsibilities:

- connect to the device
- poll or subscribe to device data
- decode protocol-specific payloads
- expose device identity and capability metadata (discovery manifest)
- emit normalized runtime events (state, availability, diagnostics) to the core
- translate commands into device-native operations
- surface device-specific failures to the core

### Central core

This tier is shared across all device microservices as a reusable Rust library or set of crates.

Responsibilities:

- runtime bootstrap
- configuration loading
- structured logging
- OpenTelemetry instrumentation
- health reporting
- lifecycle management
- converting device-side events into canonical internal messages
- orchestrating publish calls to the event bus adapter
- binding device communication to the selected event bus adapter
- enforcing a small, consistent execution model across all services
- providing a reusable service lifecycle runner so new device services only implement behavior callbacks
- providing shared state emission filtering primitives so services can suppress duplicate/noise updates consistently

The core is the main place where common engineering concerns live. Device-specific logic should stay out of it.

### Event bus adapter

This tier translates canonical internal messages into a concrete bus contract.

Examples:

- Home Assistant discovery over MQTT
- Sparkplug over MQTT
- other bus contracts added later

Responsibilities:

- publish discovery metadata
- publish state updates
- publish availability and health
- consume command messages when supported
- encode payloads for the selected bus protocol
- own adapter-specific connection configuration conventions (for example env-to-config mapping for HA MQTT)

The adapter is replaceable. A device implementation should not need to know whether it is publishing through a Home Assistant MQTT model, Sparkplug, or another bus protocol.

For adapters that multiplex multiple devices through one client session, availability may be
modeled at the node or service level rather than per device. In that case, multiple discovered
devices can share the same availability topic while keeping distinct discovery and state topics.

## Why one microservice per device

One device per microservice is an explicit design choice.

Benefits:

- failure isolation between devices
- simpler deployment and restart behavior
- easier protocol experimentation
- clear ownership of telemetry and logs
- per-device scalability
- straightforward security boundaries

Tradeoffs:

- higher service count
- more deployment metadata
- more pressure to keep runtime overhead small

Because service count is expected to be high, the implementation must stay lean.

## Resource footprint expectations

Each device microservice should aim for:

- a single async runtime process
- no embedded database by default
- small memory usage
- minimal idle CPU activity
- bounded reconnect and retry behavior
- shared crates for common functionality rather than duplicated code

Heavy orchestration or large in-process frameworks are out of scope for the device microservice layer.

## System topology

:::mermaid
flowchart LR
	subgraph Devices[Physical and logical devices]
		D1[Device A]
		D2[Device B]
		D3[Device C]
	end

	subgraph Services[Per-device microservices]
		S1[Device Service A]
		S2[Device Service B]
		S3[Device Service C]
	end

	BUS[(Central Message Bus)]

	subgraph Consumers[Platform consumers]
		UI[UI or clients]
		REG[Registry or digital twin]
		OBS[Observability pipeline]
		AUTO[Automation or rules engines]
	end

	D1 --> S1
	D2 --> S2
	D3 --> S3

	S1 <--> BUS
	S2 <--> BUS
	S3 <--> BUS

	BUS --> UI
	BUS --> REG
	BUS --> OBS
	BUS --> AUTO
:::

## Per-device service structure

:::mermaid
flowchart TD
	subgraph Service[Single device microservice]
		COMM[Device communication]
		CORE[Central core]
		ADAPTER[Event bus adapter]
	end

	DEVICE[Specific device]
	BUS[(Central message bus)]

	DEVICE <--> COMM
	COMM <--> CORE
	CORE <--> ADAPTER
	ADAPTER <--> BUS
:::

## Data flow

The normal read path is:

1. the device communication tier receives device data
2. the central core converts that into canonical internal events
3. the event bus adapter publishes discovery, state, and availability messages
4. downstream consumers subscribe and synchronize their own state

The normal command path is:

1. a consumer publishes a command onto the message bus
2. the event bus adapter receives and decodes the command
3. the central core validates and routes it
4. the device communication tier sends the device-native operation

For the ESPHome native API bridge service, step 4 maps capability commands onto ESPHome native API entity commands and forwards them over the device session.

## Message categories

The central message bus is the synchronization backbone. At minimum, the architecture expects these message categories:

- discovery
- state
- commands
- availability
- service health
- telemetry

These categories are logical contracts. A specific adapter may map them onto different topic structures or payload shapes.

## Discovery model

Discovery metadata is device-driven and adapter-encoded.

The device communication tier is the source of truth for "what this device is" and
"what capabilities this device exposes". The device integration decides when to emit
discovery (for example immediately on initialization, or only after it is fully ready),
and the adapter translates it to protocol-specific messages.

For a Home Assistant MQTT adapter, the adapter is responsible for:

- publishing retained discovery payloads
- publishing device availability
- publishing entity state topics, including protocol metadata such as source observation timestamps when available
- subscribing to command topics where relevant

Current HA MQTT mapping detail:

- state publishes use a JSON envelope with a `value` field plus protocol metadata fields such as `ts`
- discovery payloads include a `value_template` so Home Assistant reads the entity state from `value`
- the same state topic is reused as `json_attributes_topic`, and `json_attributes_template` explicitly maps `ts` into attributes without changing the canonical device contract

For a Sparkplug adapter, the adapter is responsible for:

- publishing birth and death certificates
- publishing metric updates
- exposing command handling in the Sparkplug model

The device communication tier should not contain Home Assistant-specific or Sparkplug-specific topic logic. That belongs in the adapter.

Likewise, the core should not contain integration-specific topic or payload rules. It should
only orchestrate lifecycle and call the adapter with canonical messages.

## Canonical internal contract

Inside the microservice, the central core should expose a canonical internal model that is independent of the external bus contract.

That model should include concepts such as:

- device identity
- capabilities or entities
- observed state changes
- commands
- availability
- diagnostics

This separation prevents protocol-specific bus concerns from leaking into device-specific code.

## Service lifecycle

### Overview

A device microservice goes through four distinct phases:

1. **Startup** — core and adapter initialize, bus connection is established
2. **Bootstrap** — discovery, availability, and initial state are published concurrently
3. **Steady state** — device monitoring loop runs, state changes are published continuously
4. **Shutdown** — clean or unclean, with different availability semantics for each

### Startup phase

:::mermaid
sequenceDiagram
	participant Comm as Device communication
	participant Core as Central core
	participant Adapter as Event bus adapter
	participant Bus as Central message bus

	Core->>Core: Initialize logging and telemetry
	Core->>Adapter: Create adapter with config (broker host, node id, ...)
	Adapter->>Bus: Register last will on availability topic (payload: offline, retained)
	Note over Adapter,Bus: Last will is registered BEFORE the connection is established
	Adapter->>Bus: Connect
	Bus-->>Adapter: Connection established
	Core->>Comm: Initialize device integration
	Comm-->>Core: Device integration ready
:::

The last will and testament must be registered as part of the connect options, before the
connection is established. The broker holds it and fires it automatically if the connection
is lost unexpectedly.

Device communication initialization happens only after the bus adapter connection is established.

### Bootstrap phase

After startup is complete and the integration is ready, the following can happen concurrently:

:::mermaid
sequenceDiagram
	participant Comm as Device communication
	participant Core as Central core
	participant Adapter as Event bus adapter
	participant Bus as Central message bus

	par Discovery (when integration decides it is ready)
		Comm->>Core: Emit discovery event (identity + capabilities)
		Core->>Adapter: publish_discovery(device, capabilities)
		Adapter->>Bus: Publish retained config payload per capability
	and Availability
		Comm->>Core: Emit availability event (available = true)
		Core->>Adapter: publish_availability(available = true)
		Adapter->>Bus: Publish retained "online" on availability topic
	and Initial state
		Comm->>Core: Read current device state
		Core->>Adapter: publish_state(capability, value)
		Adapter->>Bus: Publish current state per capability
	and Device loop
		Comm->>Comm: Start polling loop and/or external subscriptions
	end
:::

The order within the concurrent block does not need to be serialized. Discovery can happen
as soon as the integration has enough metadata, and availability/initial state can be in-flight
at the same time. Downstream consumers such as Home Assistant handle retained messages
independently.

The important ownership rule is preserved:

- device communication defines discovery content and decides when to emit discovery
- core orchestrates lifecycle and forwards canonical events to the adapter
- adapter maps canonical messages to bus-specific topics and payloads

### Steady state

The device monitoring loop runs for the lifetime of the process. It reacts to device-side
changes and translates them into bus messages.

:::mermaid
sequenceDiagram
	participant Dev as Physical device
	participant Comm as Device communication
	participant Core as Central core
	participant Adapter as Event bus adapter
	participant Bus as Central message bus

	loop Device monitoring
		Dev->>Comm: State change or poll response
		Comm->>Core: Normalized state event
		Core->>Adapter: publish_state(capability, value)
		Adapter->>Bus: Publish state update
		Comm->>Core: Availability change (available = true/false)
		Core->>Adapter: publish_availability(available = true/false)
		Adapter->>Bus: Publish retained availability update
	end
:::

### Clean shutdown

On receiving a shutdown signal (SIGTERM or SIGINT), the service:

1. stops the device monitoring loop
2. explicitly publishes `offline` to the availability topic (retained)
3. disconnects from the bus

:::mermaid
sequenceDiagram
	participant Core as Central core
	participant Comm as Device communication
	participant Adapter as Event bus adapter
	participant Bus as Central message bus

	Core->>Core: Receive shutdown signal (SIGTERM / SIGINT)
	Core->>Comm: Stop device monitoring loop
	Core->>Adapter: publish_availability(Offline)
	Adapter->>Bus: Publish retained "offline" on availability topic
	Adapter->>Bus: Disconnect cleanly
:::

### Unclean disconnect

If the process crashes or the network is lost without a clean shutdown, the broker fires the
last will that was registered during startup.

:::mermaid
sequenceDiagram
	participant Bus as Central message bus
	participant Cons as Downstream consumers

	Note over Bus: Process crash or network loss detected
	Bus->>Bus: Fire last will: "offline" on availability topic (retained)
	Bus-->>Cons: Deliver "offline" availability
:::

### Last will vs explicit offline

These two mechanisms cover different failure modes and must both be in place:

| Scenario | Who publishes offline | How |
|---|---|---|
| Clean shutdown | The service itself | Explicit publish before disconnect |
| Crash or network loss | The broker | Last will registered at connect time |

The availability topic must always be retained so that any consumer subscribing after a
disconnect immediately receives the current offline status without waiting for the next event.

## Service boundaries

### Device communication owns

- protocol sessions
- device-specific parsing
- device-specific command execution
- reconnect behavior for the device protocol

### Central core owns

- shared runtime patterns
- common configuration model
- OpenTelemetry SDK initialization and resource attributes
- OTLP export configuration (endpoint, enable/disable, export interval)
- health and readiness model
- canonical event model
- adapter binding

### Observability pipeline owns

- receiving OTLP signals from services
- exposing Prometheus-compatible `/metrics` endpoint for development inspection
- persisting or forwarding OTEL logs for development and production analysis
- forwarding or fan-out to production observability backends
- decoupling metric scrape concerns from service HTTP servers

### Event bus adapter owns

- bus-specific topic naming
- payload encoding and decoding
- retained discovery behavior where applicable
- bus-specific availability semantics
- command subscription semantics

## Deployment posture

This architecture is intended for distributed deployment.

That means:

- device services may run on different hosts
- all services synchronize through the central bus
- the bus is the main shared integration surface
- no direct peer-to-peer coupling is required between device services

This keeps scaling simple and preserves isolation.

For observability, each service exports telemetry to an external OpenTelemetry Collector over OTLP.
The collector is responsible for exposing a Prometheus scrape endpoint (`/metrics`) so the
service process itself does not need to host an HTTP endpoint. Logs follow the same pattern:
they are exported over OTLP to the collector and inspected through a collector-managed sink
rather than through a service-local log API.

Downstream telemetry backends behind the collector are a deployment concern and are out of
scope for the core service architecture.

## Non-goals for the device service layer

The following concerns are not part of the per-device service architecture itself:

- dashboards and end-user UI
- automation authoring UX
- long-term analytics storage
- fleet orchestration details
- centralized policy engines

Those can be built as separate consumers on top of the bus.

## Summary

The platform is a Rust-based distributed system built around a central message bus.

Each device is represented by a dedicated low-footprint microservice composed of:

1. device communication
2. central core
3. event bus adapter

This keeps device protocol logic isolated, keeps cross-cutting engineering concerns shared, and allows event bus contracts such as Home Assistant MQTT discovery or Sparkplug to evolve independently.

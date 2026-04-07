# Home Services

Home Services is a Rust workspace for designing a distributed home automation platform built from small device-focused microservices.

The current workspace has been reorganized around the architecture in `docs/architecture.md`:

- Rust-based implementation
- distributed microservice model
- central message bus for event-driven communication and state synchronization
- one lightweight microservice per device
- separation between device communication, central core, and event bus adapters

## Workspace layout

- `crates/hs-contracts`: canonical shared types for discovery, state, commands, and availability.
- `crates/hs-core`: reusable runtime and orchestration helpers for device services.
- `crates/hs-eventbus`: event bus abstraction traits.
- `crates/hs-eventbus-mqtt-ha`: Home Assistant over MQTT event bus adapter.
- `crates/hs-service-device-demo`: a minimal demo device microservice composed from the shared crates.
- `crates/hs-service-device-esphome`: ESPHome native API microservice that discovers ESPHome entities at runtime and republishes them through the canonical Home Assistant MQTT adapter.
- `crates/hs-service-device-tplink-hs110`: TP-Link HS110 local-LAN microservice for relay control and realtime power metrics over the native TP-Link protocol.
- `docs/architecture.md`: canonical architecture document.
- `.devcontainer/docker-compose.yml`: local Mosquitto broker, OpenTelemetry Collector, Grafana LGTM, and InfluxDB for development.

## Current status

- The crate layout now follows the target architecture rather than the earlier prototype.
- The event bus adapter is scaffolded around Home Assistant MQTT concepts.
- The demo service is intentionally minimal and proves the crate boundaries.
- Registry, gateway, and richer device communication crates are expected to be added back as bus consumers or concrete device implementations.

## Protocol and design references

The Home Assistant-over-MQTT conventions in this workspace are based on the following upstream sources:

- Home Assistant MQTT discovery docs: <https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery>
- Home Assistant MQTT switch platform docs (command/state payload behavior): <https://www.home-assistant.io/integrations/switch.mqtt>

Supporting local infrastructure and observability references:

- Eclipse Mosquitto documentation: <https://mosquitto.org/documentation>
- OpenTelemetry documentation: <https://opentelemetry.io/docs>
- Grafana LGTM stack overview: <https://grafana.com/oss/lgtm/>

## Dev container workflow

The workspace includes a dev container under `.devcontainer/`.

What it does:

- builds a Rust development container with the toolchain and common native dependencies
- starts Mosquitto, an OpenTelemetry Collector, Grafana LGTM, and InfluxDB as sidecar services inside the same Compose project
- forwards the API and broker ports back to the host editor
- caches Cargo registry, git dependencies, and the workspace `target/` directory across rebuilds

Recommended host setup:

- WSL2-backed container engine
- VS Code Dev Containers extension
- repository stored in the Linux filesystem if you are using WSL2 heavily

### Clean WSL setup (Podman)

If your WSL distribution is clean, install Podman tooling first:

```bash
sudo apt-get update
sudo apt-get install -y podman podman-compose uidmap slirp4netns fuse-overlayfs
podman --version
podman-compose --version
```

The workspace includes VS Code settings in `.vscode/settings.json` to point Dev Containers at Podman and podman-compose.

Open the workspace in the container:

1. Run the VS Code command `Dev Containers: Reopen in Container`.
2. Wait for the initial container build and `cargo fetch` post-create step.
3. Start the demo service:

   ```bash
   cargo run -p hs-service-device-demo
   ```

Optional validation inside WSL before opening VS Code in-container:

```bash
podman-compose -f .devcontainer/docker-compose.yml config
```

Inside the dev container:

- the MQTT broker is reachable as `mosquitto:1883`
- the OTLP HTTP endpoint is reachable as `http://otel-collector:4318`
- InfluxDB is reachable as `http://influxdb:8086`
- Grafana is available at `http://localhost:3000` (`admin` / `admin`)
- Grafana auto-provisions an `InfluxDB` datasource (Flux, org/bucket `home-services`)
- Grafana auto-provisions the dashboard `Home Services / Demo Service Overview`

## Run locally without the dev container

1. Start the local infrastructure:

   ```powershell
   podman-compose -f .devcontainer/docker-compose.yml up -d mosquitto otel-collector lgtm influxdb
   ```

2. Start the demo device service:

   ```powershell
   $env:HS_OTEL_ENABLED = "true"
   $env:OTEL_EXPORTER_OTLP_ENDPOINT = "http://127.0.0.1:4318"
   cargo run -p hs-service-device-demo
   ```

3. Open Grafana LGTM:

   ```powershell
   Start-Process http://127.0.0.1:3000
   ```

4. Open dashboard `Home Services / Demo Service Overview` to inspect demo temperature, switch, and availability data from InfluxDB.

## Run the ESPHome native API service

Set the ESPHome API endpoint and start:

```bash
export HS_DEVICE_ID="esphome-living-room-01"
export HS_DEVICE_NAME="ESPHome Living Room"
export ESPHOME_API_HOST="192.168.2.57"
export ESPHOME_API_PORT="6053"
# Optional when your ESPHome node uses API encryption
# export ESPHOME_API_ENCRYPTION_KEY="base64-noise-psk"

cargo run -p hs-service-device-esphome
```

## Run the TP-Link HS110 service

Set the HS110 endpoint and start. The service will read HS110 sysinfo and auto-fill
device identity from the plug (alias/model/mac/deviceId) when `HS_DEVICE_*`
overrides are not provided.

```bash
export HS110_HOST="192.168.2.107"
export HS110_PORT="9999"

cargo run -p hs-service-device-tplink-hs110
```

Optional timeout tuning:

```bash
export HS110_TIMEOUT_MS="3000"
```

Optional identity overrides (if you want fixed IDs/names instead of sysinfo-derived values):

```bash
export HS_SERVICE_ID="device-kitchen-plug"
export HS_DEVICE_ID="tplink-kitchen-plug-01"
export HS_DEVICE_NAME="Kitchen Plug"
export HS_DEVICE_MODEL="HS110(FR)"
export HS_DEVICE_MANUFACTURER="TP-Link"
```

## Service ports

- Mosquitto MQTT broker: `1883`
- Mosquitto WebSocket listener: `9001`
- OTLP gRPC receiver (collector): `4317`
- OTLP HTTP receiver (collector): `4318`
- Grafana LGTM UI: `3000`
- InfluxDB HTTP API: `8086`

## Next design increments

- Add concrete device communication crates
- Add command subscriptions and command routing
- Add bus consumers such as registry, gateway, and automation services
- Implement a real MQTT transport layer in the Home Assistant adapter

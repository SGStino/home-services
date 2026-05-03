# Home Assistant MQTT Discovery Protocol

Reference document for implementing `hs-eventbus-mqtt-ha`.

## Overview

Home Assistant (HA) can automatically discover devices and entities if a service publishes a specially structured retained MQTT message to a well-known topic. Once HA receives that message, it registers the entity and subscribes to its state and command topics.

The protocol has three moving parts:

1. **Config** — retained discovery payload telling HA what the entity is
2. **State** — live value published whenever the entity changes
3. **Availability** — retained online/offline signal for the MQTT client session or node

---

## Topic structure

### Config (discovery)

```
<discovery_prefix>/<component>/<node_id>/<object_id>/config
```

| Segment            | Description                                                      |
|--------------------|------------------------------------------------------------------|
| `discovery_prefix` | default `homeassistant`, configurable per HA install             |
| `component`        | entity type: `sensor`, `binary_sensor`, `switch`, `light`, etc. |
| `node_id`          | optional grouping identifier, usually the bridge or service name |
| `object_id`        | unique identifier for this specific entity                       |

Example:
```
homeassistant/sensor/hs-node-dev/living_room_sensor_01_temperature/config
```

### State

```
hs/state/<node_id>/<device_id>/<capability_id>
```

Example:
```
hs/state/hs-node-dev/living-room-sensor-01/temperature
```

### Availability

```
hs/availability/<node_id>/<session_id>
```

Example:
```
hs/availability/hs-node-dev/hs_adapter_hs_node_dev_1742230400123
```

### Command (optional, writable entities only)

```
hs/command/<node_id>/<device_id>/<capability_id>
```

Example:
```
hs/command/hs-node-dev/my-switch-01/state
```

---

## Config payload

The config payload is a JSON document published as a **retained** message.
Publishing an empty payload (`""`) to the config topic removes the entity from HA.

### Sensor example

```json
{
  "name": "Temperature",
  "unique_id": "hs-node-dev_living-room-sensor-01_temperature",
  "device_class": "temperature",
  "unit_of_measurement": "°C",
  "state_topic": "hs/state/hs-node-dev/living-room-sensor-01/temperature",
  "value_template": "{{ value_json.value }}",
  "json_attributes_topic": "hs/state/hs-node-dev/living-room-sensor-01/temperature",
  "json_attributes_template": "{{ {'ts': value_json.ts} | tojson }}",
  "availability_topic": "hs/availability/hs-node-dev/hs_adapter_hs_node_dev_1742230400123",
  "payload_available": "online",
  "payload_not_available": "offline",
  "device": {
    "identifiers": ["hs_living-room-sensor-01"],
    "name": "Living Room Sensor",
    "manufacturer": "Home Services",
    "model": "demo-sensor"
  }
}
```

### Binary sensor example

```json
{
  "name": "Occupancy",
  "unique_id": "hs-node-dev_living-room-sensor-01_occupancy",
  "device_class": "occupancy",
  "state_topic": "hs/state/hs-node-dev/living-room-sensor-01/occupancy",
  "value_template": "{{ value_json.value }}",
  "json_attributes_topic": "hs/state/hs-node-dev/living-room-sensor-01/occupancy",
  "json_attributes_template": "{{ {'ts': value_json.ts} | tojson }}",
  "payload_on": "true",
  "payload_off": "false",
  "availability_topic": "hs/availability/hs-node-dev/hs_adapter_hs_node_dev_1742230400123",
  "payload_available": "online",
  "payload_not_available": "offline",
  "device": {
    "identifiers": ["hs_living-room-sensor-01"],
    "name": "Living Room Sensor",
    "manufacturer": "Home Services",
    "model": "demo-sensor"
  }
}
```

### Switch example

```json
{
  "name": "Power",
  "unique_id": "hs-node-dev_my-switch-01_state",
  "state_topic": "hs/state/hs-node-dev/my-switch-01/state",
  "value_template": "{{ value_json.value }}",
  "json_attributes_topic": "hs/state/hs-node-dev/my-switch-01/state",
  "json_attributes_template": "{{ {'ts': value_json.ts} | tojson }}",
  "command_topic": "hs/command/hs-node-dev/my-switch-01/state",
  "payload_on": "ON",
  "payload_off": "OFF",
  "availability_topic": "hs/availability/hs-node-dev/hs_adapter_hs_node_dev_1742230400123",
  "payload_available": "online",
  "payload_not_available": "offline",
  "device": {
    "identifiers": ["hs_my-switch-01"],
    "name": "My Switch",
    "manufacturer": "Home Services",
    "model": "demo-switch"
  }
}
```

---

## Key fields reference

| Field                   | Required | Description                                                        |
|-------------------------|----------|--------------------------------------------------------------------|
| `name`                  | yes      | Human-readable entity name shown in HA                            |
| `unique_id`             | yes      | Stable unique identifier. Must never change for the same entity.  |
| `state_topic`           | yes      | MQTT topic HA subscribes to for live values                       |
| `value_template`        | recommended | Extracts the entity state from the JSON state envelope         |
| `json_attributes_topic` | recommended | Reuses the state topic so timestamp metadata becomes entity attributes |
| `json_attributes_template` | optional | Explicitly selects which JSON fields become attributes (for example `ts`) |
| `command_topic`         | no       | Only for writable entities (switches, lights, etc.)               |
| `availability_topic`    | recommended | Topic HA monitors for online/offline                           |
| `payload_available`     | no       | Payload meaning online, default `online`                          |
| `payload_not_available` | no       | Payload meaning offline, default `offline`                        |
| `device_class`          | no       | Semantic class: `temperature`, `humidity`, `occupancy`, etc.      |
| `unit_of_measurement`   | no       | Unit string displayed in HA: `°C`, `%`, `lux`, etc.              |
| `device`                | recommended | Groups multiple entities under a single device in HA UI        |

---

## device block

Multiple entities belong to the same HA device when they share the same `identifiers` list.

```json
"device": {
  "identifiers": ["hs_living-room-sensor-01"],
  "name": "Living Room Sensor",
  "manufacturer": "Home Services",
  "model": "demo-sensor",
  "sw_version": "0.1.0"
}
```

`identifiers` is what HA uses to group entities. It must be stable and unique per physical device.

---

## Availability

The availability topic carries a plain string payload.

| Publish                            | Effect in HA              |
|------------------------------------|---------------------------|
| `online` (retained)                | entity shows as available |
| `offline` (retained)               | entity shows as unavailable |
| last-will set to `offline`         | HA marks unavailable if MQTT connection drops |

The MQTT client should:
- set its **last-will** to `offline` on its session availability topic before connecting
- publish `online` (retained) immediately after connecting
- publish `offline` (retained) on clean shutdown

When one MQTT client represents multiple HA devices, those devices can all reference the same
session availability topic. In this implementation, `session_id` defaults to `MQTT_CLIENT_ID`
and can be overridden via `MQTT_AVAILABILITY_SESSION`.

This avoids rollout races in deployments: a new pod publishes retained discovery that points HA
entities at the new session topic, so a late offline/LWT from an old pod only affects the old
session topic and does not mark the new instance unavailable.

---

## State values

State payloads are JSON objects so protocol metadata can travel with the live value.
Home Assistant reads the entity state via `value_template` and stores the remaining fields as attributes.

Example state payload:

```json
{
  "value": 21.5,
  "ts": 1742230400123
}
```

| Field                  | Description                                           |
|------------------------|-------------------------------------------------------|
| `value`                | The entity state HA should treat as the live value    |
| `ts`                   | Source observation timestamp in Unix milliseconds     |

For button entities, no `state_topic` is published because HA models them as stateless actions.

---

## Message retention

| Topic type    | Should be retained? | Reason                                           |
|---------------|---------------------|--------------------------------------------------|
| config        | yes                 | HA re-subscribes on restart and must get config  |
| availability  | yes                 | HA must know current status without waiting      |
| state         | optional            | retained state lets HA show value immediately    |
| command       | no                  | commands are one-shot actions, not state         |

---

## QoS recommendations

| Message type  | QoS  |
|---------------|------|
| config        | 1    |
| availability  | 1    |
| state         | 0 or 1 depending on importance |
| command       | 1    |

---

## Entity removal

To remove an entity from HA, publish an empty retained message to the config topic:

```
Topic:   homeassistant/sensor/hs-node-dev/living_room_sensor_01_temperature/config
Payload: (empty)
Retain:  true
```

---

## Supported component types

| Component       | Readable | Writable | Notes                          |
|-----------------|----------|----------|--------------------------------|
| `sensor`        | yes      | no       | numeric or string values       |
| `binary_sensor` | yes      | no       | on/off boolean                 |
| `switch`        | yes      | yes      | toggle on/off                  |
| `light`         | yes      | yes      | brightness, color, etc.        |
| `climate`       | yes      | yes      | thermostat control             |
| `cover`         | yes      | yes      | blinds, garage doors           |
| `button`        | no       | yes      | press action                   |
| `number`        | yes      | yes      | settable numeric value         |
| `select`        | yes      | yes      | dropdown choice                |
| `text`          | yes      | yes      | settable string                |

Initial implementation will focus on `sensor`, `binary_sensor`, and `switch`.

---

## Implementation checklist for hs-eventbus-mqtt-ha

- [ ] MQTT client setup with configurable host/port/client-id
- [ ] Last-will registration on node availability topic before connect
- [ ] Publish `online` to node availability topic after connect (retained)
- [ ] Publish config payload for each capability (retained, QoS 1)
- [ ] Publish state updates (QoS 0 or 1)
- [ ] Subscribe to command topics for writable entities
- [ ] Publish `offline` to node availability topic on clean shutdown (retained)
- [ ] Publish empty config payload to remove entities on deregister

#!/usr/bin/env python3
"""Publish strict Sparkplug B DCMD boolean commands for the demo service.

This tool builds the protobuf payload manually (no extra Python deps required)
and publishes it with mosquitto_pub using binary stdin.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import time


SPARKPLUG_DATATYPE_BOOLEAN = 11


def _varint(value: int) -> bytes:
    if value < 0:
        raise ValueError("varint requires non-negative integer")

    out = bytearray()
    while True:
        to_write = value & 0x7F
        value >>= 7
        if value:
            out.append(to_write | 0x80)
        else:
            out.append(to_write)
            break
    return bytes(out)


def _field_key(field_number: int, wire_type: int) -> bytes:
    return _varint((field_number << 3) | wire_type)


def _field_varint(field_number: int, value: int) -> bytes:
    return _field_key(field_number, 0) + _varint(value)


def _field_len_delimited(field_number: int, value: bytes) -> bytes:
    return _field_key(field_number, 2) + _varint(len(value)) + value


def build_metric_bytes(metric_name: str, enabled: bool) -> bytes:
    # Sparkplug Metric fields used:
    # 1: name (string)
    # 4: datatype (uint32) -> Boolean = 11
    # 14: boolean_value (bool)
    payload = bytearray()
    payload += _field_len_delimited(1, metric_name.encode("utf-8"))
    payload += _field_varint(4, SPARKPLUG_DATATYPE_BOOLEAN)
    payload += _field_varint(14, 1 if enabled else 0)
    return bytes(payload)


def build_payload_bytes(metric_name: str, enabled: bool, seq: int | None) -> bytes:
    # Sparkplug Payload fields used:
    # 1: timestamp (uint64)
    # 2: metrics (repeated Metric message)
    # 3: seq (uint64)
    now_ms = int(time.time() * 1000)
    metric_bytes = build_metric_bytes(metric_name, enabled)

    payload = bytearray()
    payload += _field_varint(1, now_ms)
    payload += _field_len_delimited(2, metric_bytes)
    if seq is not None:
        payload += _field_varint(3, seq)

    return bytes(payload)


def sanitize(value: str) -> str:
    out = []
    for ch in value:
        if ch.isalnum():
            out.append(ch.lower())
        else:
            out.append("_")
    return "".join(out)


def build_topic(group_id: str, edge_node_id: str, device_id: str) -> str:
    return (
        f"spBv1.0/{sanitize(group_id)}/DCMD/"
        f"{sanitize(edge_node_id)}/{sanitize(device_id)}"
    )


def publish(
    host: str,
    port: int,
    topic: str,
    payload_bytes: bytes,
) -> None:
    cmd = [
        "mosquitto_pub",
        "-h",
        host,
        "-p",
        str(port),
        "-t",
        topic,
        "-s",
    ]

    result = subprocess.run(cmd, input=payload_bytes, check=False)
    if result.returncode != 0:
        raise RuntimeError(f"mosquitto_pub failed with exit code {result.returncode}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Publish Sparkplug DCMD ON/OFF boolean command via protobuf payload."
    )
    parser.add_argument(
        "state",
        choices=["on", "off"],
        help="Requested switch state.",
    )
    parser.add_argument("--host", default="mosquitto", help="MQTT broker host.")
    parser.add_argument("--port", type=int, default=1883, help="MQTT broker port.")
    parser.add_argument("--group", default="home-services", help="Sparkplug group id.")
    parser.add_argument("--edge", default="hs-node-dev", help="Sparkplug edge node id.")
    parser.add_argument(
        "--device", default="living-room-node-01", help="Sparkplug device id."
    )
    parser.add_argument(
        "--metric",
        default="power",
        help="Metric name to command (must match registered command capability).",
    )
    parser.add_argument(
        "--seq",
        type=int,
        default=None,
        help="Optional sequence number (uint64).",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    enabled = args.state == "on"

    topic = build_topic(args.group, args.edge, args.device)
    payload_bytes = build_payload_bytes(args.metric, enabled, args.seq)

    try:
        publish(args.host, args.port, topic, payload_bytes)
    except Exception as exc:  # pragma: no cover - runtime error path
        print(f"error: {exc}", file=sys.stderr)
        return 1

    print(f"published state={args.state.upper()} topic={topic} bytes={len(payload_bytes)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

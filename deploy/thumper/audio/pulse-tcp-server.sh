#!/usr/bin/env bash
# Tower5810: expose PipeWire Pulse TCP on LAN for Thumper tunnels.
set -euo pipefail

LISTEN_IP="${LISTEN_IP:-10.0.0.113}"
PORT="${PULSE_TCP_PORT:-4713}"
ACL="${PULSE_TCP_ACL:-10.0.0.0/24}"
MODULE_NAME=module-native-protocol-tcp

unload_ours() {
  # Unload all native-protocol-tcp modules (idempotent re-start)
  pactl list modules short 2>/dev/null | awk -v m="$MODULE_NAME" '$2 == m { print $1 }' | while read -r id; do
    pactl unload-module "$id" 2>/dev/null || true
  done
}

cmd="${1:-start}"
case "$cmd" in
  start)
    if ! command -v pactl >/dev/null; then
      echo "pactl not found" >&2
      exit 1
    fi
    # Wait for pipewire-pulse
    for _ in $(seq 1 30); do
      pactl info >/dev/null 2>&1 && break
      sleep 0.5
    done
    unload_ours
    id=$(pactl load-module "$MODULE_NAME" \
      "port=$PORT" \
      "listen=$LISTEN_IP" \
      "auth-ip-acl=$ACL" \
      "auth-anonymous=1")
    echo "Loaded $MODULE_NAME id=$id listen=$LISTEN_IP:$PORT acl=$ACL"
    ;;
  stop)
    unload_ours
    echo "Stopped Pulse TCP server modules"
    ;;
  status)
    ss -ltn | grep -E ":$PORT\\b" || echo "not listening on $PORT"
    pactl list modules short 2>/dev/null | grep "$MODULE_NAME" || echo "module not loaded"
    ;;
  *)
    echo "Usage: $0 start|stop|status" >&2
    exit 1
    ;;
esac

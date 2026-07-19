#!/usr/bin/env bash
# Thumper: tunnel Tower default source/sink as tower_mic / tower_speakers.
set -euo pipefail

TOWER_HOST="${TOWER_HOST:-10.0.0.113}"
PORT="${PULSE_TCP_PORT:-4713}"
SERVER="tcp:${TOWER_HOST}:${PORT}"
SRC_NAME="${TOWER_MIC_NAME:-tower_mic}"
SNK_NAME="${TOWER_SPEAKERS_NAME:-tower_speakers}"

wait_pulse() {
  for _ in $(seq 1 40); do
    pactl info >/dev/null 2>&1 && return 0
    sleep 0.5
  done
  echo "pipewire-pulse not ready" >&2
  return 1
}

wait_server() {
  for _ in $(seq 1 40); do
    if pactl -s "$SERVER" info >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.5
  done
  echo "cannot reach Pulse server $SERVER" >&2
  return 1
}

unload_named() {
  # Unload tunnel modules we own (by matching argument strings)
  pactl list modules short 2>/dev/null | while read -r id name args; do
    case "$name" in
      module-tunnel-source|module-tunnel-sink)
        if [[ "$args" == *"$SERVER"* ]] || [[ "$args" == *"$SRC_NAME"* ]] || [[ "$args" == *"$SNK_NAME"* ]]; then
          pactl unload-module "$id" 2>/dev/null || true
        fi
        ;;
    esac
  done
}

cmd="${1:-start}"
case "$cmd" in
  start)
    wait_pulse
    wait_server
    unload_named
    # Omit remote "source=" / "sink=" → use Tower defaults (switch mic in Tower Sound settings)
    sid=$(pactl load-module module-tunnel-source \
      "server=$SERVER" \
      "source_name=$SRC_NAME" \
      "source_properties=device.description=Tower_microphone")
    kid=$(pactl load-module module-tunnel-sink \
      "server=$SERVER" \
      "sink_name=$SNK_NAME" \
      "sink_properties=device.description=Tower_speakers")
    echo "tower_mic module=$sid  tower_speakers module=$kid  via $SERVER"
    pactl list short sources | grep -E "$SRC_NAME" || true
    pactl list short sinks | grep -E "$SNK_NAME" || true
    ;;
  stop)
    unload_named
    echo "tunnels stopped"
    ;;
  status)
    pactl list short sources 2>/dev/null | grep -E "$SRC_NAME" || echo "no $SRC_NAME"
    pactl list short sinks 2>/dev/null | grep -E "$SNK_NAME" || echo "no $SNK_NAME"
    pactl -s "$SERVER" info 2>&1 | head -5 || echo "server unreachable"
    ;;
  *)
    echo "Usage: $0 start|stop|status" >&2
    exit 1
    ;;
esac

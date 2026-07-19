#!/usr/bin/env bash
set -euo pipefail
NAME="${HA_CONTAINER_NAME:-mcfloater-homeassistant}"

if systemctl --user is-active --quiet mcfloater-homeassistant.service 2>/dev/null \
  || systemctl --user is-enabled --quiet mcfloater-homeassistant.service 2>/dev/null; then
  systemctl --user stop mcfloater-homeassistant.service
  echo "Home Assistant systemd unit stopped."
  # leave enabled so next ha-up / reboot still has the unit
fi

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -qx "$NAME"; then
    docker stop "$NAME" 2>/dev/null || true
    echo "Home Assistant container stopped ($NAME)."
  fi
fi

PIDFILE="${HA_PIDFILE:-$HOME/Data/mcfloater/homeassistant.pid}"
if [[ -f "$PIDFILE" ]]; then
  pid="$(cat "$PIDFILE")"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    sleep 1
    kill -9 "$pid" 2>/dev/null || true
    echo "Home Assistant nohup process stopped (pid $pid)."
  fi
  rm -f "$PIDFILE"
fi

#!/usr/bin/env bash
# Start Home Assistant for McFloater master node on Thumper.
# Prefer systemd user unit (auto-restart). Fall back to docker or nohup venv.
set -euo pipefail

export HA_CONFIG="${HA_CONFIG:-$HOME/Data/homeassistant}"
export HA_VENV="${HA_VENV:-$HOME/Data/homeassistant-venv}"
NAME="${HA_CONTAINER_NAME:-mcfloater-homeassistant}"
IMAGE="${HA_IMAGE:-ghcr.io/home-assistant/home-assistant:stable}"
UNIT_SRC="$(cd "$(dirname "$0")" && pwd)/mcfloater-homeassistant.service"
UNIT_DST="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/mcfloater-homeassistant.service"

mkdir -p "$HA_CONFIG" "$HOME/Data/mcfloater"

docker_usable() {
  command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1
}

install_unit() {
  mkdir -p "$(dirname "$UNIT_DST")"
  if [[ -f "$UNIT_SRC" ]]; then
    cp -f "$UNIT_SRC" "$UNIT_DST"
    systemctl --user daemon-reload
  fi
}

ensure_venv_hass() {
  if [[ -x "$HA_VENV/bin/hass" ]]; then
    return 0
  fi
  echo "ERROR: hass not found at $HA_VENV/bin/hass" >&2
  echo "  Install Core first (see deploy/thumper/README.md)." >&2
  exit 1
}

# Prefer systemd so Restart=always survives UI restart + crashes
if command -v systemctl >/dev/null 2>&1 && systemctl --user status >/dev/null 2>&1; then
  ensure_venv_hass
  install_unit
  # Stop any stray nohup instance using our pidfile
  if [[ -f "$HOME/Data/mcfloater/homeassistant.pid" ]]; then
    old="$(cat "$HOME/Data/mcfloater/homeassistant.pid" || true)"
    if [[ -n "${old:-}" ]] && kill -0 "$old" 2>/dev/null; then
      # only kill if not already under systemd
      if ! systemctl --user is-active --quiet mcfloater-homeassistant.service 2>/dev/null; then
        kill "$old" 2>/dev/null || true
        sleep 1
        kill -9 "$old" 2>/dev/null || true
      fi
    fi
    rm -f "$HOME/Data/mcfloater/homeassistant.pid"
  fi
  systemctl --user enable mcfloater-homeassistant.service
  systemctl --user restart mcfloater-homeassistant.service
  echo "Home Assistant (systemd user) → http://$(hostname -f 2>/dev/null || hostname):8123"
  echo "  status: systemctl --user status mcfloater-homeassistant"
  echo "  logs:   journalctl --user -u mcfloater-homeassistant -f"
  echo "  stop:   $(cd "$(dirname "$0")" && pwd)/ha-down.sh"
  if ! loginctl show-user "$USER" -p Linger 2>/dev/null | grep -q 'Linger=yes'; then
    echo
    echo "NOTE: user lingering is off — service may stop on logout."
    echo "      Once (needs sudo):  sudo loginctl enable-linger $USER"
  fi
  exit 0
fi

if docker_usable; then
  if docker ps -a --format '{{.Names}}' | grep -qx "$NAME"; then
    docker start "$NAME"
  else
    docker pull "$IMAGE"
    docker run -d --name "$NAME" --restart unless-stopped --network host \
      -e "TZ=${TZ:-America/Indiana/Indianapolis}" \
      -v "$HA_CONFIG:/config" -v /etc/localtime:/etc/localtime:ro \
      "$IMAGE"
  fi
  echo "Home Assistant (Docker) → http://$(hostname -f 2>/dev/null || hostname):8123"
  exit 0
fi

# Last resort: nohup (no auto-restart)
ensure_venv_hass
nohup "$HA_VENV/bin/hass" --config "$HA_CONFIG" >>"$HOME/Data/mcfloater/homeassistant.log" 2>&1 &
echo $! >"$HOME/Data/mcfloater/homeassistant.pid"
echo "Home Assistant (nohup, NO auto-restart) pid=$(cat "$HOME/Data/mcfloater/homeassistant.pid")"
echo "  Prefer: fix systemd --user, then re-run this script."

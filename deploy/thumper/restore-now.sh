#!/usr/bin/env bash
# Run ON Thumper: bash thumper-mcfloater-restore.sh
# Or from Tower:  scp this file && ssh user@thumper.local bash -s < thisfile
set -euo pipefail
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

echo "======== McFloater Thumper restore ========"
cd "$HOME/Documents/McFloater/deploy/thumper" 2>/dev/null || {
  echo "ERROR: ~/Documents/McFloater/deploy/thumper missing"
  exit 1
}

chmod +x bootstrap-ubuntu-26.04.sh ha-up.sh install-piper.sh install-wyoming-whisper.sh 2>/dev/null || true

echo "==> bootstrap (apt, rust, whisper, cargo build, brain unit)"
./bootstrap-ubuntu-26.04.sh

# shellcheck disable=SC1091
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

echo "==> Home Assistant"
./ha-up.sh || true

if [[ ! -x "$HOME/Data/mcfloater/piper/piper" ]]; then
  echo "==> Piper"
  ./install-piper.sh || true
else
  echo "==> Piper already present"
fi

if ! command -v ollama >/dev/null 2>&1; then
  echo "==> Installing Ollama (linux amd64 tarball)"
  tmp=$(mktemp -d)
  cd "$tmp"
  # latest stable pattern — adjust if API changes
  curl -fsSL https://ollama.com/install.sh | sh || {
    echo "install.sh failed; try manual: https://github.com/ollama/ollama/releases"
  }
  cd - >/dev/null
  rm -rf "$tmp"
fi

# user unit for ollama if missing
if [[ ! -f "$HOME/.config/systemd/user/ollama.service" ]]; then
  mkdir -p "$HOME/.config/systemd/user"
  cat > "$HOME/.config/systemd/user/ollama.service" <<'UNIT'
[Unit]
Description=Ollama (user)
After=network-online.target

[Service]
ExecStart=%h/.local/bin/ollama serve
Restart=on-failure
RestartSec=3
Environment=HOME=%h
Environment=PATH=%h/.local/bin:/usr/local/bin:/usr/bin

[Install]
WantedBy=default.target
UNIT
  # prefer system install path if present
  if [[ -x /usr/local/bin/ollama ]]; then
    sed -i 's|%h/.local/bin/ollama|/usr/local/bin/ollama|' "$HOME/.config/systemd/user/ollama.service"
  elif [[ -x /usr/bin/ollama ]]; then
    sed -i 's|%h/.local/bin/ollama|/usr/bin/ollama|' "$HOME/.config/systemd/user/ollama.service"
  fi
fi

systemctl --user daemon-reload
systemctl --user enable --now ollama.service 2>/dev/null || true
# if unit failed, start bare
if ! curl -sf -m 2 http://127.0.0.1:11434/ >/dev/null 2>&1; then
  nohup ollama serve >"$HOME/Data/mcfloater/ollama.log" 2>&1 &
  sleep 2
fi

echo "==> Ollama models (chat + instruct)"
ollama pull llama3.1:8b || ollama pull llama3.2:3b || true
ollama pull mistral || true

systemctl --user enable --now mcfloater-brain.service || true
systemctl --user enable --now mcfloater-wyoming-whisper.service 2>/dev/null || true
systemctl --user restart mcfloater-brain.service || true

echo
echo "======== status ========"
systemctl --user --no-pager status mcfloater-brain mcfloater-homeassistant mcfloater-wyoming-whisper ollama 2>&1 | head -60 || true
echo
echo "--- health ---"
curl -sS -m 5 http://127.0.0.1:8750/health 2>&1 || echo "brain DOWN"
echo
curl -sS -m 3 -o /dev/null -w "HA http %{http_code}\n" http://127.0.0.1:8123/ || true
curl -sS -m 3 http://127.0.0.1:11434/api/tags 2>&1 | head -c 400 || echo "ollama DOWN"
echo
echo "Env: ~/Data/mcfloater/mcfloater.env — need HA_TOKEN set for HA hands"
grep -E '^(HA_URL|HA_TOKEN|MCFLOATER_OLLAMA|MCFLOATER_PIPER|MCFLOATER_WYOMING|XAI)' "$HOME/Data/mcfloater/mcfloater.env" 2>/dev/null | sed -E 's/(TOKEN|KEY)=.*/\1=<redacted>/' || true
echo "======== done ========"

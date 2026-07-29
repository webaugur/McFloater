#!/usr/bin/env bash
# Install Wyoming Faster-Whisper as a user systemd service (no Docker required).
# Shared STT for HA Assist + McFloater brain (MCFLOATER_WYOMING_STT=127.0.0.1:10300).
set -euo pipefail
DATA="${MCFLOATER_WYOMING_DATA:-$HOME/Data/mcfloater/wyoming}"
mkdir -p "$DATA/whisper"
MODEL="${MCFLOATER_WHISPER_MODEL:-tiny-int8}"

echo "Installing wyoming-faster-whisper (user pip)…"
python3 -m pip install --user -U wyoming-faster-whisper

BIN="$HOME/.local/bin/wyoming-faster-whisper"
if [[ ! -x "$BIN" ]]; then
  echo "expected $BIN after pip install" >&2
  exit 1
fi

UNIT_SRC="$(cd "$(dirname "$0")" && pwd)/mcfloater-wyoming-whisper.service"
mkdir -p "$HOME/.config/systemd/user"
cp "$UNIT_SRC" "$HOME/.config/systemd/user/"
# Patch model into unit if env overrides
if [[ "$MODEL" != "tiny-int8" ]]; then
  sed -i "s/--model tiny-int8/--model ${MODEL}/" \
    "$HOME/.config/systemd/user/mcfloater-wyoming-whisper.service"
fi

systemctl --user daemon-reload
systemctl --user enable --now mcfloater-wyoming-whisper.service
sleep 1
systemctl --user --no-pager status mcfloater-wyoming-whisper.service || true
echo
echo "Wyoming Whisper: tcp://127.0.0.1:10300"
echo "  env: MCFLOATER_WYOMING_STT=127.0.0.1:10300"
echo "  HA:  Integrations → Wyoming Protocol → 127.0.0.1:10300"

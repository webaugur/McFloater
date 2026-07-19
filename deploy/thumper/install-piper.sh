#!/usr/bin/env bash
# Install Piper neural TTS binary + a default English voice into ~/Data/mcfloater/piper
# Run on Thumper (or any brain host). Idempotent.
set -euo pipefail

ROOT="${MCFLOATER_PIPER_DIR:-$HOME/Data/mcfloater/piper}"
PIPER_VER="${PIPER_VERSION:-2023.11.14-2}"
# Young-adult male / neutral (ryan). Override with PIPER_VOICE_* if you prefer.
# Other good males: joe, lessac, alan, northern_english_male
VOICE_LANG="${PIPER_VOICE_LANG:-en}"
VOICE_LOCALE="${PIPER_VOICE_LOCALE:-en_US}"
VOICE_NAME="${PIPER_VOICE_NAME:-ryan}"
VOICE_QUALITY="${PIPER_VOICE_QUALITY:-medium}"
VOICE_FILE="${VOICE_LOCALE}-${VOICE_NAME}-${VOICE_QUALITY}"

mkdir -p "$ROOT"
cd "$ROOT"

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64) PIPER_ARCH=x86_64 ;;
  aarch64|arm64) PIPER_ARCH=aarch64 ;;
  *) echo "Unsupported arch: $ARCH" >&2; exit 1 ;;
esac

BIN_TGZ="piper_linux_${PIPER_ARCH}.tar.gz"
BIN_URL="https://github.com/rhasspy/piper/releases/download/${PIPER_VER}/${BIN_TGZ}"

need_bin=0
if [[ ! -x "$ROOT/piper" || -d "$ROOT/piper" ]]; then
  need_bin=1
fi

if [[ "$need_bin" -eq 1 ]]; then
  echo "Downloading Piper ${PIPER_VER} (${PIPER_ARCH})…"
  curl -fL --retry 3 -o "$BIN_TGZ" "$BIN_URL"
  rm -rf "$ROOT/_extract"
  mkdir -p "$ROOT/_extract"
  tar -xzf "$BIN_TGZ" -C "$ROOT/_extract"
  # Archive layout: either files at top or a single `piper/` directory of bin+libs
  if [[ -x "$ROOT/_extract/piper/piper" ]]; then
    SRC="$ROOT/_extract/piper"
  elif [[ -x "$ROOT/_extract/piper" && -f "$ROOT/_extract/piper" ]]; then
    SRC="$ROOT/_extract"
  else
    # search
    SRC="$(find "$ROOT/_extract" -type f -name piper -perm -111 | head -1 | xargs -r dirname)"
  fi
  if [[ -z "${SRC:-}" || ! -d "$SRC" ]]; then
    echo "ERROR: could not find piper binary in tarball" >&2
    find "$ROOT/_extract" -maxdepth 3 -type f | head -40 >&2
    exit 1
  fi
  # Copy bin + shared libs next to models (keep existing .onnx)
  cp -a "$SRC"/. "$ROOT/"
  # Ensure executable name is $ROOT/piper (file, not dir)
  if [[ -d "$ROOT/piper" && -x "$ROOT/piper/piper" ]]; then
    # nested leftover
    cp -f "$ROOT/piper/piper" "$ROOT/piper.exe"
    rm -rf "$ROOT/piper"
    mv "$ROOT/piper.exe" "$ROOT/piper"
  fi
  chmod +x "$ROOT/piper"
  rm -rf "$ROOT/_extract" "$BIN_TGZ"
fi

if [[ ! -x "$ROOT/piper" || -d "$ROOT/piper" ]]; then
  echo "ERROR: piper binary missing at $ROOT/piper" >&2
  ls -la "$ROOT" >&2
  exit 1
fi

MODEL="$ROOT/${VOICE_FILE}.onnx"
CFG="$ROOT/${VOICE_FILE}.onnx.json"
HF_BASE="https://huggingface.co/rhasspy/piper-voices/resolve/main/${VOICE_LANG}/${VOICE_LOCALE}/${VOICE_NAME}/${VOICE_QUALITY}"

if [[ ! -f "$MODEL" || $(stat -c%s "$MODEL" 2>/dev/null || echo 0) -lt 1000000 ]]; then
  echo "Downloading voice ${VOICE_FILE}…"
  curl -fL --retry 3 -o "$MODEL" "${HF_BASE}/${VOICE_FILE}.onnx?download=true"
  curl -fL --retry 3 -o "$CFG" "${HF_BASE}/${VOICE_FILE}.onnx.json?download=true"
fi

# Quick synthesis smoke
echo "Piper install smoke…"
echo "McFloater Piper online." | "$ROOT/piper" --model "$MODEL" --output_file /tmp/mcfloater-piper-smoke.wav
ls -lh /tmp/mcfloater-piper-smoke.wav

echo
echo "Piper installed (lab default voice stem: ${VOICE_FILE}):"
echo "  bin:   $ROOT/piper"
echo "  model: $MODEL"
echo
echo "Locked defaults for ~/Data/mcfloater/mcfloater.env:"
echo "  MCFLOATER_PIPER_BIN=$ROOT/piper"
echo "  MCFLOATER_PIPER_MODEL=$MODEL"
echo "  MCFLOATER_TTS_ENGINE=piper"
echo
echo "Optional voices (re-run with PIPER_VOICE_NAME=joe|lessac|alan|…):"
echo "  joe, lessac, alan, northern_english_male, alba, amy"
echo
echo "Then: systemctl --user restart mcfloater-brain"
echo "From Tower: mcfloater say \"Hello from the lab.\"   # uses brain default model"
echo "            mcfloater voices                      # list SAM + Piper options"

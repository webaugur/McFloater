#!/usr/bin/env bash
# Bootstrap McFloater brain host on a fresh Ubuntu 26.04 (resolute) install.
# Safe to re-run. Does NOT overwrite mcfloater.env or Home Assistant config.
#
# Usage (on Thumper):
#   cd ~/Documents/McFloater/deploy/thumper
#   ./bootstrap-ubuntu-26.04.sh
#
# Then open a new shell (or source ~/.cargo/env) and:
#   systemctl --user restart mcfloater-brain
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DATA="${MCFLOATER_DATA:-$HOME/Data/mcfloater}"
HA_CONFIG="${HA_CONFIG:-$HOME/Data/homeassistant}"

echo "==> McFloater Thumper bootstrap (Ubuntu 26.04)"
echo "    repo: $ROOT"
echo "    data: $DATA"

# --- OS packages (full set for McFloater + deploy tooling on Thumper) ---
# Validated package names against Ubuntu 26.04 (resolute).
echo "==> apt packages (needs sudo)"
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  \
  `# --- toolchain / fetch ---` \
  build-essential \
  pkg-config \
  cmake \
  ninja-build \
  git \
  curl \
  wget \
  rsync \
  ca-certificates \
  zstd \
  xz-utils \
  unzip \
  jq \
  \
  `# --- Rust native: SAM (cc + bindgen), OpenSSL crates, udev ---` \
  clang \
  libclang-dev \
  llvm-dev \
  libssl-dev \
  libudev-dev \
  \
  `# --- Audio: cpal (ALSA) + PipeWire stack (Ubuntu 26.04 default) ---` \
  libasound2-dev \
  libpulse-dev \
  pulseaudio-utils \
  pipewire \
  pipewire-pulse \
  pipewire-audio-client-libraries \
  wireplumber \
  libpipewire-0.3-dev \
  libspa-0.2-dev \
  \
  `# --- Python: HA Core venv, tinytuya, wyoming-faster-whisper ---` \
  python3 \
  python3-full \
  python3-pip \
  python3-venv \
  python3-dev \
  libffi-dev \
  libjpeg-dev \
  libopenjp2-7 \
  libtiff6 \
  zlib1g-dev \
  libsqlite3-dev \
  autoconf \
  libblas3 \
  liblapack3 \
  libgomp1 \
  \
  `# --- media helpers (Piper smoke, wav, ffmpeg filters) ---` \
  ffmpeg

# Optional: GPU userspace often already installed; do not force proprietary drivers here.
# sudo ubuntu-drivers autoinstall   # if nvidia-smi missing

# --- linger so user services survive logout ---
if [[ "$(loginctl show-user "$USER" -p Linger --value 2>/dev/null || true)" != "yes" ]]; then
  echo "==> enable-linger (user services after logout)"
  sudo loginctl enable-linger "$USER"
fi

# --- PATH ---
mkdir -p "$HOME/.local/bin"
if ! grep -q '\.local/bin' "$HOME/.bashrc" 2>/dev/null; then
  echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
fi
if ! grep -q '\.cargo/env' "$HOME/.bashrc" 2>/dev/null; then
  echo '[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"' >> "$HOME/.bashrc"
fi
export PATH="$HOME/.local/bin:$PATH"

# --- Rust ---
if ! command -v cargo >/dev/null 2>&1; then
  echo "==> install rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env"

# --- Data dirs ---
mkdir -p "$DATA" "$HA_CONFIG" "$DATA/wyoming/whisper" "$DATA/piper"

# --- env file ---
ENV_FILE="$DATA/mcfloater.env"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "==> seeding $ENV_FILE from env.example"
  cp "$(dirname "$0")/env.example" "$ENV_FILE"
  chmod 600 "$ENV_FILE"
  echo "    EDIT $ENV_FILE — set HA_TOKEN at minimum"
else
  echo "==> keeping existing $ENV_FILE"
fi

# --- tinytuya (optional device key dump) ---
python3 -m pip install --user -U tinytuya 2>/dev/null || \
  python3 -m pip install --user -U --break-system-packages tinytuya
if [[ ! -x "$HOME/.local/bin/tinytuya" ]]; then
  cat > "$HOME/.local/bin/tinytuya" <<'EOF'
#!/usr/bin/env bash
exec python3 -m tinytuya "$@"
EOF
  chmod +x "$HOME/.local/bin/tinytuya"
fi

# --- Wyoming Faster-Whisper (user service, no Docker) ---
if [[ -x "$(dirname "$0")/install-wyoming-whisper.sh" ]]; then
  echo "==> wyoming-faster-whisper"
  bash "$(dirname "$0")/install-wyoming-whisper.sh" || true
fi

# --- Piper (if binary missing) ---
if [[ ! -x "$DATA/piper/piper" ]]; then
  echo "==> piper missing under $DATA/piper — run ./install-piper.sh"
else
  echo "==> piper binary present"
fi

# --- Ollama (user-local tarball style if not installed) ---
if ! command -v ollama >/dev/null 2>&1; then
  echo "==> Ollama not in PATH. Install manually if needed:"
  echo "    https://github.com/ollama/ollama/releases (linux-amd64 tar.zst)"
  echo "    install bin/ollama → ~/.local/bin and lib/ollama → ~/.local/lib/ollama"
  echo "    then: ollama serve (user systemd unit) && ollama pull llama3.1:8b && ollama pull mistral"
else
  echo "==> ollama present: $(ollama --version 2>/dev/null || true)"
fi

# --- Build McFloater brain binary ---
echo "==> cargo build -p mcfloater --release"
cd "$ROOT"
cargo build -p mcfloater --release
install -D "$ROOT/target/release/mcfloater" "$HOME/.local/bin/mcfloater"

# --- systemd user units ---
UNIT_DIR="$HOME/.config/systemd/user"
mkdir -p "$UNIT_DIR"
cp "$(dirname "$0")/mcfloater-brain.service" "$UNIT_DIR/"
# HA unit if present
if [[ -f "$(dirname "$0")/mcfloater-homeassistant.service" ]]; then
  cp "$(dirname "$0")/mcfloater-homeassistant.service" "$UNIT_DIR/"
fi
systemctl --user daemon-reload
systemctl --user enable mcfloater-brain.service
systemctl --user restart mcfloater-brain.service || true
systemctl --user enable mcfloater-homeassistant.service 2>/dev/null || true
# don't force-start HA if venv missing — user runs ha-up.sh

echo
echo "==> PipeWire (Ubuntu 26.04 default audio)"
systemctl --user is-active pipewire pipewire-pulse wireplumber || true
echo "    Desk mic/speakers for Floaty stay on Tower; Thumper only needs PipeWire if you use local audio here."
echo
echo "==> Done. Checks:"
echo "    systemctl --user status mcfloater-brain"
echo "    curl -sS http://127.0.0.1:8750/health"
echo "    From Tower: export MCFLOATER_BRAIN_URL=http://thumper.local:8750 && mcfloater health"
echo
echo "Still manual if missing:"
echo "  1) HA:  cd deploy/thumper && ./ha-up.sh"
echo "  2) Ollama models: ollama pull llama3.1:8b && ollama pull mistral"
echo "  3) Grok: uncomment XAI_API_KEY in $ENV_FILE"
echo "  4) NVIDIA CUDA for Whisper/Ollama GPU (optional): ubuntu-drivers / nvidia-cuda-toolkit as needed"

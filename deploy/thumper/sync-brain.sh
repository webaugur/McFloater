#!/usr/bin/env bash
# Sync McFloater brain sources Tower → Thumper, release-build, install, restart.
# Run from Tower after editing brain (or linked) code.
set -euo pipefail

HOST="${MCFLOATER_THUMPER_HOST:-thumper.local}"
REMOTE_ROOT="${MCFLOATER_THUMPER_ROOT:-Documents/McFloater}"
LOCAL_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "==> sync-brain: $LOCAL_ROOT → ${HOST}:${REMOTE_ROOT}"

CRATES=(
  mcfloater-core
  mcfloater-audio
  mcfloater-stt
  mcfloater-tts
  mcfloater-lipsync
  mcfloater-render
  mcfloater-ha
  mcfloater-brain
)

for c in "${CRATES[@]}"; do
  rsync -a --delete \
    --exclude target \
    --exclude '*.glb' \
    "${LOCAL_ROOT}/crates/${c}/" \
    "${HOST}:${REMOTE_ROOT}/crates/${c}/"
  echo "  synced crates/${c}"
done

rsync -a --delete --exclude target \
  "${LOCAL_ROOT}/apps/mcfloater/" \
  "${HOST}:${REMOTE_ROOT}/apps/mcfloater/"
echo "  synced apps/mcfloater"

rsync -a \
  "${LOCAL_ROOT}/Cargo.toml" \
  "${LOCAL_ROOT}/Cargo.lock" \
  "${HOST}:${REMOTE_ROOT}/"

if [[ -d "${LOCAL_ROOT}/ffi/sam" ]]; then
  rsync -a "${LOCAL_ROOT}/ffi/sam/" "${HOST}:${REMOTE_ROOT}/ffi/sam/"
  echo "  synced ffi/sam"
fi

echo "==> build + install + restart on ${HOST}"
ssh "${HOST}" bash -s <<'REMOTE'
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
cd "${MCFLOATER_THUMPER_ROOT:-$HOME/Documents/McFloater}"
# Prefer Data path if that's the real tree (Thumper sometimes uses Data/Documents)
if [[ ! -f Cargo.toml && -f "$HOME/Data/Documents/McFloater/Cargo.toml" ]]; then
  cd "$HOME/Data/Documents/McFloater"
fi
pwd
cargo build -p mcfloater --release
install -D target/release/mcfloater "$HOME/.local/bin/mcfloater"
systemctl --user restart mcfloater-brain.service
sleep 1
systemctl --user is-active mcfloater-brain.service
curl -sS -X POST http://127.0.0.1:8750/v1/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"Hello!"}'
echo
REMOTE

echo "==> sync-brain done"

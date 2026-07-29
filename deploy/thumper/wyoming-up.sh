#!/usr/bin/env bash
# Start Wyoming Whisper (and optional Piper) for HA Assist + McFloater STT.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
DATA="${MCFLOATER_WYOMING_DATA:-$HOME/Data/mcfloater/wyoming}"
mkdir -p "$DATA/whisper" "$DATA/piper"
export MCFLOATER_WYOMING_DATA="$DATA"
cd "$ROOT"
echo "Wyoming data: $DATA"
# Prefer Compose v2 plugin; fall back to docker-compose v1.
if docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE=(docker-compose)
else
  echo "need docker compose or docker-compose" >&2
  exit 1
fi
"${COMPOSE[@]}" -f wyoming-compose.yml up -d wyoming-whisper
echo "Whisper Wyoming on tcp://127.0.0.1:10300"
echo "Add in HA: Integrations → Wyoming Protocol → 127.0.0.1:10300"
echo "Optional Piper: ${COMPOSE[*]} -f wyoming-compose.yml --profile piper up -d"

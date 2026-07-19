# Thumper as McFloater master node

**Host:** `thumper.local` (`user@thumper.local`, `10.0.0.30`)  
**Desk display:** `Tower5810` (`10.0.0.113`, FirePro / Vulkan, monitor on `:0`)

**Role:** Always-on **Floaty McFloater brain + hands** — STT, Ollama dialog, Home Assistant (KMC/Tuya plugs), later video call / telepresence.  
**Not** the desk GUI host — the face runs on Tower GPUs.

This is **not** a DragonSDR feature. Radio lives in `~/Documents/DragonSDR`. GPU inventory / dual-TITAN locks: IndianaDell `docs/thumper-gpu.md`.

## Ownership

| Component | Where | Path / notes |
|-----------|--------|--------------|
| McFloater source | both (same repo) | `~/Documents/McFloater` |
| **GUI / avatar (Bevy)** | **Tower5810** | FirePro Vulkan, `DISPLAY=:0` |
| **SAM TTS + lip-sync** | **Tower5810** | CPU formant synth; stays next to the face for low latency |
| **Mic / speakers (desk)** | **Tower5810** | Local PipeWire/ALSA; no tunnel required for desk C&C |
| **STT + LLM (Ollama)** | **Thumper** | TITAN Xp CUDA when needed |
| **HA + Tuya / KMC** | **Thumper** | `~/Data/homeassistant`, REST token on Thumper only |
| McFloater brain runtime | Thumper | `~/Data/mcfloater` |
| Deploy scripts | Thumper | `McFloater/deploy/thumper/` |

## Target architecture (split)

```text
┌──────────────── Tower5810 (face) ─────────────────┐
│  Mic → capture PCM                                 │
│  Bevy GUI (FirePro Vulkan) ← visemes / state       │
│  SAM TTS → local speakers                          │
│         │  WebSocket / HTTP JSON (LAN)             │
└─────────┼──────────────────────────────────────────┘
          ▼
┌──────────────── Thumper (brain + hands) ───────────┐
│  STT  →  intents / Ollama  →  reply text           │
│                 └→ HA REST → Tuya Local → KMC lamps│
│  Home Assistant :8123                              │
└────────────────────────────────────────────────────┘
```

**Why this split**

| Machine | Strength | McFloater use |
|---------|----------|---------------|
| **Tower5810** | Monitor + multi FirePro (OpenGL/Vulkan), desk audio | GUI, SAM, lip-sync, capture |
| **Thumper** | Always-on, TITAN Xp, HA already here | STT, Ollama, HA token/entities |

**Display is easy:** run the GUI binary **natively on Tower** (`cargo run` / systemd user unit with `DISPLAY=:0`). No X11-forward, no VNC, no “remote GPU” path.

**HA stays on Thumper:** the brain process holds `HA_URL` + `HA_TOKEN`. Tower never needs the long-lived token if all device control goes through the brain API.

## Control plane (planned)

| Port | Service |
|------|---------|
| **8123** | Home Assistant UI / API (Thumper) |
| **8750** | McFloater **brain** — WebSocket + small HTTP (Thumper; bind LAN) |
| **4713/tcp** | Optional Pulse tunnel (Tower server ← Thumper client) |
| (later) | WebRTC for “call me”; RTP multi-host |

### Brain API sketch (JSON over WebSocket)

Tower → Thumper:

- `audio.chunk` — PCM frames for STT (or upload whole utterance)
- `text.user` — typed / debug text as if spoken
- `ha.call` — optional direct service call once auth is on brain (prefer brain-side intents)

Thumper → Tower:

- `state` — `idle | listening | thinking | speaking`
- `transcript` — partial / final STT
- `reply.text` — dialog line for SAM on Tower
- `ha.event` — entity changed (optional, for HUD)
- `error`

SAM runs on **Tower** so the avatar and audio share one clock for lip-sync. Ollama/STT can take hundreds of ms on Thumper without stalling the render loop.

## Lab audio paths

### A — Desk C&C (preferred with GUI on Tower)

Mic and speakers are **local to Tower**. Only **features** (text/audio chunks + replies) cross the LAN. Pulse tunnel not required.

### B — Headless brain on Thumper (no GUI)

Use the existing Pulse tunnels so Thumper hears Tower mic and plays to Tower speakers:

| On Thumper | Role |
|------------|------|
| **`tower_mic`** | Capture = Tower’s **default source** |
| **`tower_speakers`** | Playback = Tower’s **default sink** |

```bash
# Tower
systemctl --user enable --now tower-pulse-tcp.service
# Thumper
systemctl --user enable --now thumper-audio-tunnel.service
parecord --device=tower_mic /tmp/test.wav
```

Details: [`deploy/thumper/audio/README.md`](../deploy/thumper/audio/README.md)

## Phases (master node)

| Phase | Work | Status |
|-------|------|--------|
| A | Docs + `deploy/thumper` in this repo | In progress |
| B | HA Core + local Tuya/KMC plugs | Half (UI up; plugs not done) |
| B′ | **Brain service** on Thumper (HA client + health + chat intents) | **Scaffolded** (`mcfloater brain`) |
| C | STT on Thumper; Tower streams audio / text | Planned |
| D | Intents → lamp control via HA | Lite phrases work; Ollama later |
| 1+ | Bevy face on Tower GPUs; brain status + SAM from GUI | **Scaffolded** (`--features face` → `mcfloater face`) |
| E | Video call / reach-you | Deferred |

## Brain quick start

```bash
# Thumper — after HA long-lived token in ~/Data/mcfloater/mcfloater.env
cd ~/Documents/McFloater
cargo build -p mcfloater --release
./target/release/mcfloater brain
# or: systemctl --user enable --now mcfloater-brain.service

# Tower — client env
export MCFLOATER_BRAIN_URL=http://thumper.local:8750
mcfloater health
mcfloater states --domain switch
mcfloater toggle switch.desk_lamp
mcfloater ask turn on desk lamp    # brain intent + local SAM reply
```

HTTP surface (bind `0.0.0.0:8750` by default):

| Method | Path | Role |
|--------|------|------|
| GET | `/health` | Brain + HA token check |
| GET | `/v1/ha/states?domain=` | List entities |
| POST | `/v1/ha/turn_on` `turn_off` `toggle` | `{"entity_id":"…"}` |
| POST | `/v1/ha/call` | domain/service/entity_id |
| POST | `/v1/chat` | Simple intents → reply text |
| POST | `/v1/tts` | Natural speech (Piper) → `audio/wav` |

**Voice routing (default `auto`):** Tower tries **Piper on Thumper** first (`POST /v1/tts`, voice `en_US-ryan-medium`). If Thumper TTS is **busy** (`tts_busy` / HTTP 503), **slow**, or **offline**, Tower falls back to local **SAM** (`floaty` preset) for that line (and may prefer SAM once more after a slow call). Force with `--engine sam|brain` or `MCFLOATER_SPEECH_ENGINE`.

## Face (Tower Bevy GUI)

Bevy is **opt-in** (`--features face`) so default brain/CLI builds stay light on 16 GiB. Workspace cargo jobs are capped at 2 (`.cargo/config.toml`).

```bash
# on Tower5810 (monitor + FirePro)
export MCFLOATER_BRAIN_URL=http://thumper.local:8750
cargo build -p mcfloater --release --features face
cargo run -p mcfloater --release --features face -- face
# keys: Space = local SAM demo, A = ask brain + speak reply, Esc = quit
# still tight on RAM: CARGO_BUILD_JOBS=1 cargo build -p mcfloater --release --features face
```

If Vulkan misbehaves on the FirePros: `WGPU_BACKEND=gl mcfloater face`.

## Quick start (HA)

```bash
ssh user@thumper.local
cd ~/Documents/McFloater/deploy/thumper
./ha-up.sh
# open http://thumper.local:8123  (from Tower browser is fine)
```

Tuya Local and long-lived tokens: see `deploy/thumper/README.md`.  
Token lives in **`~/Data/mcfloater/mcfloater.env` on Thumper** only.

## Video call (Phase E — not started)

Future options: Matrix/Jitsi, LiveKit, SIP. Intent sketch: “call me” opens a WebRTC session to Tower or phone. **Do not block lamp C&C on this.**

## Related docs

- `deploy/thumper/README.md` — compose, Tuya, tokens  
- `deploy/thumper/audio/README.md` — Pulse tunnel (path B)  
- `../README.md` — McFloater product phases (SAM, Bevy, Ollama)  
- IndianaDell `docs/thumper-gpu.md` — NVIDIA power/clock locks  

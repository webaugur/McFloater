# Floaty McFloater

A Max Headroom parody avatar that listens, thinks, and speaks in the glitchy voice of a rogue TV host.

**Floaty McFloater** uses:
- **Rust** for real-time audio, speech synthesis, lip sync, and 3D rendering
- **SAM** (C64 formant synth) for realtime robotic speech — no neural models
- **Python** for local LLM dialog (Ollama sidecar, Phase 3)
- **C** for the SAM speech engine

Speech is parametric synthesis: phonemes → formants → PCM, the same class of engine as 1970s–80s hardware vocoders. Runs in milliseconds on CPU.

## Project status

| Phase | Feature | Status |
|-------|---------|--------|
| 0 | Cargo workspace + SAM TTS proof-of-life | Done |
| 1 | Bevy 3D head + CRT look **on Tower5810 GPUs** | Scaffolded (`mcfloater face`) |
| 2 | Streaming STT **on Thumper** (Tower sends audio/text) | Planned |
| 3 | Ollama dialog loop **on Thumper** | Planned |
| 4 | Lip sync (SAM phoneme timeline) **on Tower** | Planned |
| 5 | Polish + ship | Planned |
| **Master** | **Split lab:** Tower = face; Thumper = brain + HA hands | **Docs + HA half-up** |
| Master E | Video call / “call me” telepresence | Deferred |

**Lab split:** GUI/avatar on **Tower5810** (FirePro Vulkan + desk audio). AI + Home Assistant on **thumper.local** (TITAN Xp, always-on). See [`docs/thumper-master-node.md`](docs/thumper-master-node.md).

Home automation and assistant C&C live **here**, not in DragonSDR.

## Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs/)
- **Build tools**: `build-essential`, `pkg-config`
- **ALSA** (Linux audio): `libasound2-dev`

```bash
sudo apt install build-essential pkg-config libasound2-dev
```

## Build and run

Default builds **exclude Bevy** (keeps RAM down on 16 GiB Tower). Brain / SAM / HA CLI:

```bash
cargo build -p mcfloater --release
cargo run -p mcfloater --release
cargo run -p mcfloater --release -- "Hello! I'm Floaty McFloater."
```

Bevy face on local GPU (Tower5810) — opt-in feature, capped to 2 cargo jobs via `.cargo/config.toml`:

```bash
export MCFLOATER_BRAIN_URL=http://thumper.local:8750
cargo build -p mcfloater --release --features face
cargo run -p mcfloater --release --features face -- face
# Space = speak · A = ask brain · Esc = quit
# Default mesh: assets/face/T2-avatar-bevy.glb (Avaturn T2 + jaw/visemes). See assets/face/README.md
# export MCFLOATER_FACE_GLB=face/T2-avatar-with-animation-bevy.glb
# If it still OOMs: CARGO_BUILD_JOBS=1 cargo build -p mcfloater --release --features face
```

### Voice defaults (locked)

| Path | Default | Notes |
|------|---------|--------|
| **Routing** | **`auto`** | **Piper on Thumper first**, **SAM** if busy/slow/offline |
| SAM (backup) | **`floaty`** — 74/56/122/118 | Young/neutral male formant |
| Piper (primary) | **`en_US-ryan-medium`** | Young adult male neural |

```bash
mcfloater voices                          # list presets + engines
export MCFLOATER_BRAIN_URL=http://thumper.local:8750
mcfloater speak                           # default intro (Piper; SAM if Thumper loaded)
mcfloater ask                             # brain greeting reply (same intro via intent)
mcfloater --engine sam "Hello."           # force local SAM
mcfloater --engine brain "Hello."         # Piper only (no SAM backup)
mcfloater --voice classic --engine sam hi # SAM preset override
```

**Overload fallback:** concurrent Piper jobs → brain `503 tts_busy` → SAM this line;
Piper slower than `MCFLOATER_TTS_SLOW_MS` (default 3500) → **next** line uses SAM once.

Natural speech install (once on Thumper): `deploy/thumper/install-piper.sh`

## Workspace layout

```text
McFloater/
├── apps/mcfloater/          # Main binary
├── crates/
│   ├── mcfloater-audio/     # cpal capture/playback
│   ├── mcfloater-core/      # State machine + dialog loop
│   ├── mcfloater-lipsync/   # Viseme timeline from SAM phonemes
│   ├── mcfloater-render/    # Bevy 3D head (Vulkan on Radeon)
│   ├── mcfloater-stt/       # Streaming speech-to-text
│   └── mcfloater-tts/       # SAM formant synthesis
├── deploy/thumper/          # HA docker compose + master-node ops
├── docs/                    # thumper-master-node.md, …
├── ffi/sam/                 # Vendored SAM C sources
└── python/                  # Ollama bridge (Phase 3)
```

## License

MIT
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
| 1 | Bevy 3D head + CRT shader | Planned |
| 2 | Always-on streaming STT | Planned |
| 3 | Ollama dialog loop | Planned |
| 4 | Lip sync (SAM phoneme timeline) | Planned |
| 5 | Polish + ship | Planned |

## Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs/)
- **Build tools**: `build-essential`, `pkg-config`
- **ALSA** (Linux audio): `libasound2-dev`

```bash
sudo apt install build-essential pkg-config libasound2-dev
```

## Build and run

```bash
cargo build -p mcfloater --release
cargo run -p mcfloater --release
cargo run -p mcfloater --release -- "G-g-great to see you!"
```

Tune the robotic voice with SAM parameters:

```bash
cargo run -p mcfloater --release -- --speed 78 --pitch 70 --throat 115 --mouth 105 \
  "Catch the wave!"
```

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
└── ffi/sam/                 # Vendored SAM C sources
```

## License

MIT
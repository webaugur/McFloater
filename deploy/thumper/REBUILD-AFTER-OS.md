# Thumper rebuild after OS reinstall (Ubuntu 26.04)

Projects under `~/Documents/McFloater` and data under `~/Data/` survived; **user services + toolchains** usually did not.

## What we saw after crash recovery

| Item | Status after 26.04 |
|------|---------------------|
| Ubuntu | **26.04 LTS** (resolute) |
| Kernel | 7.x |
| Repo `~/Documents/McFloater` | recovered |
| `~/Data/mcfloater` (env, piper, wyoming) | recovered |
| `~/Data/homeassistant` | recovered |
| **PipeWire** | **active** (use this; Pulse is pipewire-pulse) |
| NVIDIA TITAN Xp | driver present (`nvidia-smi`) |
| Rust / cargo | **missing** — reinstall |
| User services (brain, ollama, whisper) | **inactive** |
| `loginctl linger` | **no** — enable so services survive logout |

## One-shot bootstrap

On Thumper (after network + sudo work):

```bash
cd ~/Documents/McFloater
# if repo is stale vs Tower:
#   rsync from Tower, or git pull
cd deploy/thumper
chmod +x bootstrap-ubuntu-26.04.sh
./bootstrap-ubuntu-26.04.sh
```

That installs build deps, PipeWire stack, Rust, tinytuya, Whisper user service, builds `mcfloater`, enables `mcfloater-brain`.

## Stack checklist (order)

1. **OS packages** — `./bootstrap-ubuntu-26.04.sh` installs the full APT set (clang/bindgen, ALSA, PipeWire, Python/HA, ffmpeg, …)  
2. **`loginctl enable-linger $USER`** — user systemd after reboot  
3. **Rust** — rustup (also via bootstrap)  
4. **PipeWire** — packages + user session; `pipewire-pulse` for Pulse-API clients  
5. **Home Assistant** — `./ha-up.sh` (venv or docker; config is already in `~/Data/homeassistant`)  
6. **Piper** — binary under `~/Data/mcfloater/piper` if recovered; else `./install-piper.sh`  
7. **Wyoming Whisper** — `./install-wyoming-whisper.sh`  
8. **Ollama** — reinstall binary+libs if missing; `ollama pull llama3.1:8b` && `ollama pull mistral`  
9. **Brain** — `cargo build -p mcfloater --release` && `systemctl --user enable --now mcfloater-brain`  
10. **Env** — `~/Data/mcfloater/mcfloater.env` (HA_TOKEN, Ollama, optional XAI_API_KEY)  
11. **Tower** — `MCFLOATER_BRAIN_URL=http://thumper.local:8750` and `mcfloater health`

## PipeWire vs Pulse (26.04)

Use **PipeWire** (with `pipewire-pulse` for apps that still speak Pulse API).

| Role | Where | Notes |
|------|--------|--------|
| Face mic + speakers | **Tower** | PipeWire on Tower; Floaty uses cpal default devices |
| Brain STT/TTS | **Thumper** | Whisper + Piper are file/TCP, not live desk audio |
| Optional Thumper→Tower audio tunnel | old `deploy/thumper/audio/*` | Pulse/TCP scripts; refresh only if you still need Path B |

You do **not** need the old Pulse tunnel for the default split (GUI on Tower, brain on Thumper).

## Verify

```bash
# Thumper
systemctl --user status mcfloater-brain mcfloater-wyoming-whisper ollama mcfloater-homeassistant
curl -sS http://127.0.0.1:8750/health | python3 -m json.tool

# Tower
ping -c2 thumper.local
export MCFLOATER_BRAIN_URL=http://thumper.local:8750
mcfloater health
mcfloater ask "hello"
```

## If brain health fails from Tower

Almost always **LAN or service down**, not Grok:

```bash
ping thumper.local
curl -sS -m 3 http://thumper.local:8750/health
```

`auto` speech then falls back to **SAM** with log `brain health failed`.

## NVIDIA note

Driver may work while CUDA toolkit for GPU Whisper is optional. CPU Whisper (`tiny-int8`) is fine to get unblocked; GPU later.

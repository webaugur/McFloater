# Agent instructions (McFloater / Floaty McFloater)

## Pull requests

- **Do not open, create, or draft GitHub pull requests** unless the user **explicitly** asks.
- Prefer branch push + status summary over PR creation by default.

## Product scope

**Floaty McFloater** is the lab AI assistant (Max Headroom parody): listen, think, speak (SAM formant TTS), later avatar + video call.

| Owns | Does **not** own |
|------|------------------|
| Voice C&C, STT/TTS, Ollama dialog | DragonSDR radio stack |
| Thumper **master node** deploy (brain + HA) | Tower5810 rebuild-machine (IndianaDell) |
| Home Assistant as **actuator bus** (lamps/plugs) | LingBot-Map / CUDA apps |
| Tower5810 **face** (Bevy GUI, SAM, desk audio) | — |

## Lab split (target)

| Host | Role |
|------|------|
| **Tower5810** | GUI / avatar on FirePro Vulkan, local mic/speakers, SAM TTS + lip-sync |
| **thumper.local** | Brain: STT, Ollama, HA token + device control; always-on |

Do **not** put the long-lived HA token on Tower if the brain API can own device calls.  
Do **not** X11-forward the avatar — run the GUI **natively** on Tower (`DISPLAY=:0`).

## Thumper master node

| Doc | Content |
|-----|---------|
| **`docs/thumper-master-node.md`** | Role, split architecture, phases A–E |
| **`deploy/thumper/README.md`** | HA compose, Tuya Local, tokens, mixer notes |

**Assumptions:**

- KMC smart plugs = **Tuya rebrand**, control over **LAN** (not Alexa / China cloud path).
- McFloater owns **personality + intents**; HA owns **device entities**.
- Video call is **Phase E** — do not block lamp C&C.

```bash
# HA on thumper
cd ~/Documents/McFloater/deploy/thumper && ./ha-up.sh
# http://thumper.local:8123
```

### Brain deploy (required after brain code changes)

Tower and Thumper keep **separate** `~/Documents/McFloater` trees. Editing brain code on Tower does **not** update the live service.

**Whenever you change** anything the brain binary uses (`crates/mcfloater-brain/**`, and linked crates: `mcfloater-ha`, `mcfloater-tts`, `mcfloater-core`, `mcfloater-audio`, `apps/mcfloater/**` brain path):

1. Sync source to Thumper  
2. `cargo build -p mcfloater --release` **on Thumper**  
3. `install` → `~/.local/bin/mcfloater`  
4. `systemctl --user restart mcfloater-brain`  
5. Smoke: `curl` `POST /v1/chat` and confirm reply text  

Use:

```bash
# from Tower
./deploy/thumper/sync-brain.sh
```

Do **not** claim face **A** / `mcfloater ask` is fixed until Thumper has been rebuilt and restarted.

## Related repos

- DragonSDR — SDR only; point assistant questions here  
- IndianaDell `docs/thumper-gpu.md` — NVIDIA dual-GPU power locks on Thumper  

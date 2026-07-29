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

---

## IndianaDell integration note (2026-07-, created by Grok session)

`bin/verify-indianadell.sh` (stub) was added so that
`~/Documents/IndianaDell/bin/fix-indianadell.sh`
can discover and optionally invoke McFloater verification without
hard-coding paths or making the IndianaDell compatibility check fatal
when McFloater is absent or incomplete.  The hook is deliberately
non-fatal.  Real master-node / brain / HA verification belongs in
`deploy/thumper/` or a future `tools/verify-master-node.sh`.  Keep the
filename `verify-indianadell.sh` so the IndianaDell hook continues to
find it.

## Desktop launchers (note for DragonSDR / IndianaDell sessions)

When other agents add or change `.desktop` files in `~/Applications/`,
remind them to run both:
  `~/Documents/IndianaDell/scripts/gnome/fix-nautilus-desktop-launch.sh`
  `~/Documents/IndianaDell/scripts/gnome/sync-desktop-icons.sh --dir ~/Applications`
(The first restores double-click; the second sets the branded icon via GIO metadata.)

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

### Voice stack (prefer Wyoming / HA Assist tools)

Align with Home Assistant Voice / [Wyoming](https://www.home-assistant.io/integrations/wyoming/) — same services for HA Assist and McFloater:

| Service | Role | Thumper |
|---------|------|---------|
| **wyoming-whisper** | STT | `deploy/thumper/wyoming-up.sh` → `:10300` |
| **Piper** (CLI or wyoming-piper) | TTS | already installed under `~/Data/mcfloater/piper` |
| **Ollama** | open dialog | `MCFLOATER_OLLAMA_*` |
| **HA Assist** | home control sentences | expose entities; optional Wyoming in HA UI |

Do **not** invent a one-off STT stack when Wyoming Whisper can be shared.

## Related repos

- DragonSDR — SDR only; point assistant questions here  
- IndianaDell `docs/thumper-gpu.md` — NVIDIA dual-GPU power locks on Thumper  

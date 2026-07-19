# Thumper deploy — Home Assistant + McFloater master

See also: [`docs/thumper-master-node.md`](../../docs/thumper-master-node.md)

## Home Assistant

```bash
cd ~/Documents/McFloater/deploy/thumper
./ha-up.sh
# http://thumper.local:8123
```

Config lives in **`~/Data/homeassistant`** (ZFS data pool). Container name: `mcfloater-homeassistant`.

`ha-up.sh` prefers a **systemd --user** unit (`mcfloater-homeassistant.service`) with **`Restart=always`** so UI restarts and crashes come back within a few seconds.

Then Docker (if permitted), then bare nohup (no auto-restart).

```bash
./ha-up.sh
systemctl --user status mcfloater-homeassistant
journalctl --user -u mcfloater-homeassistant -f

# survive logout (once, needs sudo password):
sudo loginctl enable-linger "$USER"
```

Optional Docker later: `sudo usermod -aG docker "$USER"` then re-login.

Optional `docker-compose.yml` is kept for hosts that have Compose v2.

```bash
docker logs -f mcfloater-homeassistant
./ha-down.sh
```

### First boot

1. Create admin user in the UI.  
2. Set timezone / location.  
3. Skip Alexa and unrelated clouds for C&C.

### KMC / Tuya plugs (local LAN)

Assumption: plugs are **Tuya rebrands**, controllable **on the LAN**. Do **not** use Alexa for automation.

1. Install **HACS** (if needed), then **Tuya Local** (preferred) or LocalTuya.  
2. One-time device IDs / local keys:
   - `tinytuya` wizard from Thumper, or  
   - temporary Tuya IoT / official Tuya integration only to harvest keys, then prefer local entities.  
3. Name entities (`switch.desk_lamp`, …), assign Areas.  
4. Create scenes: `all_off`, `lab_on`.  
5. **Prove local:** disconnect WAN (or block internet on phone) and toggle from HA.

### McFloater → HA token (Phase D / brain)

1. HA → user profile → **Security** → **Create long-lived access token**.  
2. Copy `env.example` → `~/Data/mcfloater/mcfloater.env` and set `HA_URL` / `HA_TOKEN`.  
3. Run the **brain** on Thumper (token never leaves this host):

```bash
cd ~/Documents/McFloater
cargo build -p mcfloater --release
install -D target/release/mcfloater ~/.local/bin/mcfloater
# optional user unit:
cp deploy/thumper/mcfloater-brain.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now mcfloater-brain.service
# or foreground:
mcfloater brain   # http://thumper.local:8750/health
```

4. On **Tower5810**, only set the brain URL:

```bash
export MCFLOATER_BRAIN_URL=http://thumper.local:8750
# optional: echo that into ~/Data/mcfloater/mcfloater.env or repo mcfloater.env
mcfloater health
mcfloater states --domain switch
mcfloater ask toggle switch.example
```

### Natural TTS (Piper on Thumper)

SAM stays on Tower for the glitchy Max voice. **Natural speech** runs on Thumper via [Piper](https://github.com/rhasspy/piper) and is served as WAV:

```bash
# once on Thumper
cd ~/Documents/McFloater/deploy/thumper && ./install-piper.sh
# append the printed MCFLOATER_PIPER_* lines to ~/Data/mcfloater/mcfloater.env
systemctl --user restart mcfloater-brain
```

Default voice is **en_US-ryan-medium** (young adult male). Other options:

```bash
# e.g. neutral-clear male lessac
PIPER_VOICE_LOCALE=en_US PIPER_VOICE_NAME=lessac ./install-piper.sh
# then set MCFLOATER_PIPER_MODEL to the new .onnx and restart brain
```

```bash
# from Tower
mcfloater health                    # tts_ok should be true
mcfloater say "Hello from the lab." # Piper on Thumper → play on Tower
mcfloater ask --engine brain "hello"
# or: export MCFLOATER_SPEECH_ENGINE=brain
```

Brain API: `POST /v1/tts` JSON `{"text":"…"}` → `audio/wav`.

Example raw REST toggle (same as brain uses):

```bash
source ~/Data/mcfloater/mcfloater.env
curl -s -X POST \
  -H "Authorization: Bearer $HA_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"entity_id":"switch.example"}' \
  "$HA_URL/api/services/switch/turn_on"
```

## Mixer / mic (Phase C)

Today Thumper may only show HDA PCH + NVIDIA HDMI until the mixer is connected:

```bash
cat /proc/asound/cards
arecord -l
pactl list short sources
```

McFloater STT (`mcfloater-stt`) and SAM TTS (`mcfloater-tts`) are the in-repo path; interim sidecars (faster-whisper, Piper) only if crates are not ready.

## systemd (optional)

Example user unit (install after HA is stable):

```ini
# ~/.config/systemd/user/mcfloater-homeassistant.service
[Unit]
Description=McFloater Home Assistant (docker compose)
After=docker.service

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=%h/Documents/McFloater/deploy/thumper
Environment=HA_CONFIG=%h/Data/homeassistant
ExecStart=%h/Documents/McFloater/deploy/thumper/ha-up.sh
ExecStop=%h/Documents/McFloater/deploy/thumper/ha-down.sh

[Install]
WantedBy=default.target
```

## Not in scope here

- LingBot-Map / CUDA — DragonSDR + IndianaDell GPU notes  
- Video call “call me” — Phase E in `docs/thumper-master-node.md`  

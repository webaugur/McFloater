# Lab audio tunnel (Tower5810 ↔ Thumper)

PulseAudio-over-TCP tunnels via PipeWire’s Pulse compatibility.  
**RTP multi-host is later**; this is the simple duplex link for McFloater C&C.

| Direction | Name on Thumper | Meaning |
|-----------|-----------------|---------|
| Tower mic → Thumper | **`tower_mic`** | Speak from Tower (BT headset / USB / whatever is **default source** on Tower) |
| Thumper → Tower speakers | **`tower_speakers`** | McFloater TTS / alerts play on Tower (BT / default sink) |

| Host | LAN IP (typical) | Role |
|------|------------------|------|
| Tower5810 | `10.0.0.113` | Pulse TCP **server** (`:4713`) |
| Thumper | `10.0.0.30` | Tunnel **client** |

## Install / enable

### On Tower5810

```bash
# from McFloater repo (or copy deploy/thumper/audio/)
mkdir -p ~/.config/systemd/user
cp deploy/thumper/audio/tower-pulse-tcp.service ~/.config/systemd/user/
# edit ExecStart if Tower IP is not 10.0.0.113
systemctl --user daemon-reload
systemctl --user enable --now tower-pulse-tcp.service
systemctl --user status tower-pulse-tcp.service
```

### On Thumper

```bash
mkdir -p ~/.config/systemd/user
cp ~/Documents/McFloater/deploy/thumper/audio/thumper-audio-tunnel.service ~/.config/systemd/user/
# TOWER_HOST=10.0.0.113 is the default in the unit
systemctl --user daemon-reload
systemctl --user enable --now thumper-audio-tunnel.service
pactl list short sources | grep tower
pactl list short sinks | grep tower
```

One-shot without systemd:

```bash
# Tower
./pulse-tcp-server.sh start

# Thumper
TOWER_HOST=10.0.0.113 ./thumper-tunnel.sh start
```

## Speak to McFloater / record from Thumper

1. On **Tower**, set the mic you want as **default input** (GNOME Settings → Sound → Input, or `pavucontrol` / `pactl set-default-source …`).
   - USB mic example: `alsa_input.usb-C-Media_…`
   - BT headset mic: connect headset, switch profile to **HSP/HFP** if needed, then set that source default.
2. On Thumper, apps use source **`tower_mic`**:
   ```bash
   parecord --device=tower_mic /tmp/test.wav
   # McFloater later: MCFLOATER_CAPTURE_DEVICE=tower_mic
   ```

Default source on Tower is whatever you leave selected — tunnel always follows **default** (no hard-coded BT name).

## Playback to Tower

```bash
# on Thumper
paplay --device=tower_speakers /usr/share/sounds/freedesktop/stereo/complete.oga
pactl set-default-sink tower_speakers   # optional
```

## Troubleshooting

| Symptom | Check |
|---------|--------|
| Tunnel fails to load | `systemctl --user status tower-pulse-tcp` on Tower; `ss -ltn \| grep 4713` |
| Empty/silent mic | Default source on Tower; BT needs headset profile with mic |
| Works then dies after sleep | Re-run tunnel service; both user sessions must be logged in (linger OK) |
| Wrong IP after DHCP | Set `TOWER_HOST` / `LISTEN_IP` in drop-ins or env files |

```bash
# Tower
journalctl --user -u tower-pulse-tcp -f
# Thumper
journalctl --user -u thumper-audio-tunnel -f
pactl -s tcp:10.0.0.113:4713 info
```

## Later: RTP multi-host

Replace or supplement these Pulse tunnels with PipeWire RTP/SAP or ROC when more than two hosts need the same graph. Keep this TCP pair for the McFloater master path until then.

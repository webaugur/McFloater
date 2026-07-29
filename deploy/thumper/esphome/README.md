# ESPHome for KMC / TYWE3S plugs (McFloater)

This directory contains ESPHome device configurations for flashing the KMC smart plugs (TYWE3S / ESP8266 module) to open-source firmware for local LAN control.

## Goal
Replace the stock Tuya cloud firmware with ESPHome so the plugs appear as native Home Assistant `switch` entities. Once any switch/light/scene entity exists, the McFloater brain health check (`ha_control_ok`) flips to `true` and the assistant can control the devices.

## Prerequisites on Tower5810 (flashing machine)

```bash
# Install ESPHome (one-time)
pip3 install --user esphome

# 3.3 V USB-UART adapter (FTDI, CP2102, CH340, etc. — must be 3.3 V capable)
# Never use a 5 V adapter on the TYWE3S pins.
```

## Wiring (serial flash)

Connect a 3.3 V USB serial adapter to the TYWE3S module:

| Adapter pin | TYWE3S pin     | Notes |
|-------------|----------------|-------|
| TX          | RX             | Crossed |
| RX          | TX             | Crossed |
| GND         | GND            | Common ground |
| 3.3 V       | VCC            | Or power the plug from its own USB and only connect TX/RX/GND |
| —           | GPIO0          | Pull LOW at reset to enter bootloader (most boards have a button or test pad) |

Typical sequence:
1. Power the plug (USB or bench supply).
2. Hold GPIO0 low (or press the button that pulls it low).
3. Briefly press reset (or power-cycle) while GPIO0 is still low.
4. Release GPIO0 after ~1 second — the chip is now in flash mode.

## Prepare secrets

Create `secrets.yaml` next to the device file (or use ESPHome’s dashboard):

```yaml
# deploy/thumper/esphome/secrets.yaml
wifi_ssid: "your-2.4ghz-ssid"
wifi_password: "your-password"
fallback_ap_password: "long-random-string-for-emergency-ap"
api_encryption_key: "generate-with-esphome or paste 32-byte base64 key"
ota_password: "another-long-random-string"
```

## Flash (first time — serial)

From the repo root on Tower5810:

```bash
esphome run deploy/thumper/esphome/kmc-tywe3s-plug.yaml
```

ESPHome will compile, connect over the serial port you select, and flash.

After the first successful flash, the device will connect to your Wi-Fi and appear in Home Assistant (via the native API). Subsequent updates can be done over-the-air (OTA) — no more serial cable needed.

## Verify on Thumper (brain side)

After the plug appears in HA:

```bash
curl -s http://thumper.local:8750/health | jq
```

Look for:

```json
"ha_control_ok": true,
"ha_message": "API running."
```

Once `ha_control_ok` is true, the brain will route device-control intents through Home Assistant and the plugs become usable from voice commands.

## GPIO mapping notes

The default config uses common values seen on many KMC/Tuya TYWE3S plugs:

- Relay: GPIO4 (very common)
- Button: GPIO0 (often the same pad used for flashing)
- Status LED: GPIO2 (inverted)

If the relay does not switch after flashing, you must probe the board (continuity tester or visual inspection of the relay driver transistor) and correct the pin numbers, then re-flash.

Common alternative pins seen in the wild:
- Relay on GPIO5
- Button on GPIO13 or GPIO3
- LED on GPIO16

## Adding more plugs or strips

- 3-outlet strip with power monitoring + 1 button (KMC TYWE3S/ESP-12E + BL0937): use `kmc-tywe3s-plug.yaml`
- 4-outlet power strip (ESP-12E form factor): use `esphome-12e-4outlet.yaml`

Duplicate the appropriate file, change the `name` / `friendly_name`, pick a static IP if desired, and adjust the GPIO numbers after verifying the board. The default `kmc-tywe3s-plug.yaml` now includes power monitoring (voltage, current, power, energy) for the three outlets.

## References

- TYWE3S datasheet and pinout: Tuya developer portal / ESPHome ESP8266 docs
- ESPHome TYWE3S community templates: search “TYWE3S” on esphome.io or the ESPHome Discord
- McFloater brain health endpoint: `http://thumper.local:8750/health`

Once the first plug is online and controllable, the full voice + HA loop is complete.
# Native Instruments Traktor Kontrol S4 MK2 Hardware Protocol

- **Vendor ID**: `0x17cc`
- **Product ID**: `0x1310`
- **Class**: USB Composite (Audio + HID)

---

## 1. USB Interface Architecture

| Interface | Type | Direction | Endpoint | Usage |
|---|---|---|---|---|
| **0, 1, 2** | Audio / MIDI | IN / OUT | — | Multi-channel sound card & MIDI ports |
| **3** | HID / Control | IN | `0x83` (Interrupt) | Buttons, jog wheels (32-bit delta), faders, EQs |
| **3** | HID / Control | OUT | `0x02` (Interrupt) | RGB pad LEDs, button backlights, VU meters |

---

## 2. Input Packets (Interface 3, EP 0x83)

### Packet 1: Buttons & Jog Wheels (20 bytes)
- **Bytes 1..4**: Left jog wheel 32-bit position counter
- **Bytes 5..8**: Right jog wheel 32-bit position counter
- **Byte 10**: Right load, deck switch, cue B/D, quantize, snap
- **Byte 11**: Right remix slots 1..4, loop out/in, flux, reset
- **Byte 12**: Right hotcue 1..4, shift, sync, cue, play
- **Byte 13**: Left hotcue 1..4, shift, sync, cue, play
- **Byte 14**: Left remix slots 1..4, loop out/in, flux, reset
- **Byte 15**: Left load, deck switch, cue A/C, browse encoder press, jog wheel touch detection
- **Byte 16**: FX buttons, loop/tempo encoder presses
- **Byte 17**: Headphone cue buttons, mixer buttons

### Packet 2: Sliders, EQs & Potentiometers (80 bytes)
- 4-channel mixer strips (Gain, 3-band EQ, Filter, Fader)
- Crossfader & Pitch tempo faders (12-bit ADCs normalized to 4096)
- FX unit knobs 1–3 + Dry/Wet

---

## 3. Output Packets (Interface 3, EP 0x02)

### Report 0x80: Deck LEDs & RGB Pads (64 bytes)
- RGB pads for Decks A & B with 3-byte RGB offsets (R, G, B)
- Single-byte backlights for transport, cue, loop, and FX buttons

### Report 0x81: Mixer LEDs & VU Level Meters (64 bytes)
- Channel level meters (Left/Right)
- Clip indicators

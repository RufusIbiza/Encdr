# Native Instruments Traktor Kontrol S2 MK2 Hardware Protocol

- **Vendor ID**: `0x17cc`
- **Product ID**: `0x1320`
- **Class**: USB Composite (Audio + HID)

---

## 1. USB Interface Architecture

| Interface | Type | Direction | Endpoint | Usage |
|---|---|---|---|---|
| **0, 1, 2** | Audio / MIDI | IN / OUT | — | Multi-channel sound card |
| **3** | HID / Control | IN | `0x83` (Interrupt) | Buttons, jog wheels (8-bit counters), faders |
| **3** | HID / Control | OUT | `0x02` (Interrupt) | Button LEDs, RGB pads, VU meters |

---

## 2. Input Packets (Interface 3, EP 0x83)

### Packet 1: Buttons & Jogwheels (17 bytes)
- **Byte 1**: Left jog wheel position (8-bit counter)
- **Byte 5**: Right jog wheel position (8-bit counter)
- **Byte 9**: Right play, cue, sync, shift, cues 1–4
- **Byte 10**: Jog wheel touch/press (L/R), main/booth switch, mic engage, right flux, loops
- **Byte 11**: Left play, cue, sync, shift, cues 1–4
- **Byte 12**: Remix on A/B, browse load A/B, cue A, left flux, left loop in/out
- **Byte 13**: FX unit 1/2 buttons and channel assign
- **Byte 14**: Remix slot buttons 1..4 (left and right)
- **Byte 15**: Encoder presses (Browse, Loop L/R, Move L/R)

### Packet 2: Sliders & Encoders (51 bytes)
- **Bytes 2..5**: 4-bit nibble relative encoders (Browse, Loop, Move, Gain)
- **Bytes 5..50**: 12-bit analog sliders and potentiometers:
  - Crossfader & Pitch faders (L/R)
  - Channel volume faders (L/R)
  - 3-band EQs (Hi, Mid, Low)
  - FX 1 & FX 2 knobs (Dry/Wet, 1..3)

---

## 3. Output Packets (Interface 3, EP 0x02)

### Report 0x80: Main LEDs & RGB Pads (64 bytes)
- RGB pads for Decks A & B (3-byte offsets)
- Transport, sync, cue, shift, loop backlight LEDs

### Report 0x81: Mixer VU Meters (64 bytes)
- 4-segment channel LED level meters for Left and Right channels

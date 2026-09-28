# Native Instruments Traktor Kontrol S2 MK1 Hardware Protocol

- **Vendor ID**: `0x17cc`
- **Product ID**: `0x1101`
- **Class**: USB Composite (Audio + HID)

---

## 1. USB Interface Architecture

| Interface | Type | Direction | Endpoint | Usage |
|---|---|---|---|---|
| **0, 1, 2** | Audio / MIDI | IN / OUT | — | Sound card & MIDI ports |
| **3** | HID / Control | IN | `0x83` (Interrupt) | Buttons, jog wheels (32-bit counters), faders |
| **3** | HID / Control | OUT | `0x02` (Interrupt) | Button LEDs, dual-color pads (Green/Blue), VU meters |

---

## 2. Input Packets (Interface 3, EP 0x83)

### Packet 1: Buttons & Jogwheels (16 bytes)
- **Bytes 1..4**: Left jog wheel 32-bit position counter
- **Bytes 5..8**: Right jog wheel 32-bit position counter
- **Byte 9**: Left PFL, loops, reset, FX1 buttons
- **Byte 10**: FX2 buttons, FX channel assigns
- **Byte 11**: Right PFL, loops, reset, load track, samples
- **Byte 12**: Right shift, sync, cue, play, pads 1..4
- **Byte 13**: Left shift, sync, cue, play, pads 1..4
- **Byte 14**: Encoder push switches (Browse, Gain, Loop, Move)

### Packet 2: Sliders & Encoders (52 bytes)
- **Bytes 1..4**: 4-bit nibble relative encoders (Gain, Left, Right, Browse)
- **Bytes 5..50**: 12-bit analog sliders and potentiometers:
  - FX 1 & 2 knobs + Dry/Wet
  - Jog press sensors (L/R)
  - Pitch faders (L/R)
  - 3-band EQs (Hi, Mid, Low)
  - Channel faders 1 & 2
  - Crossfader & Headphone mix

---

## 3. Output Packets (Interface 3, EP 0x02)

### Report 0x80: Button LEDs & Dual-Color Pads (62 bytes)
- Dual-color performance pads (dedicated Green and Blue LED offsets for 4 pads per deck)
- 4-segment VU meters for Left & Right channels
- Warning alert LED
- Transport, sync, cue, loop, and FX button backlights

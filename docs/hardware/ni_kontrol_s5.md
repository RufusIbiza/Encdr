# NI Kontrol S5 Hardware Reference

## Overview

| Property     | Value                                                |
| ------------ | ---------------------------------------------------- |
| Manufacturer | Native Instruments                                   |
| Product      | Kontrol S5                                           |
| VID:PID      | `0x17cc:0x1420`                                      |
| USB Speed    | High Speed (480 Mbps)                                |
| Interfaces   | 5 (Audio Out, Audio In, MIDI/DFU, HID Control, Bulk Display) |
| Screens      | 2 x 480 x 272, BGR565 big-endian                     |
| Total LEDs   | Split across 3 prefix groups: Left (0x80), Right (0x81), Mixer (0x82) |

The Traktor Kontrol S5 is a portable 4-channel DJ controller featuring dual full-color displays, 16 RGB performance pads, touch-sensitive knobs, and touchstrips.

---

## Physical Layout

The S5 shares its core deck architecture with the Kontrol D2 and S8, paired with a central 4-channel mixer. Unlike the S8, the S5 omits the dedicated remix faders/knobs and analog phono preamps, delivering a streamlined, highly responsive digital workflow.

### Deck Sections (Left & Right)
- 4.3" Full-Color Display (480x272)
- 8 Display-adjacent buttons (4 left, 4 right of screen)
- 4 Performance encoders under each display with touch detection
- 4 FX knobs with capacitive touch detection
- 4 FX buttons + FX Select button
- 8 RGB performance pads (Pads 1–8)
- 25-LED Touchstrip (bi-color: blue + orange)
- Transport: Play, Cue, Sync, Shift, Flux
- Mode: Hotcue, Loop, Freeze, Remix, Deck Select
- Navigation: Back, Capture, Edit, Browse push-encoder

### Mixer Section (Center)
- 4 Channel faders (`mixer_fader_a`..`d`, 12-bit)
- Crossfader (12-bit)
- 20 Analog knobs (Gain, Hi EQ, Mid EQ, Low EQ, Filter across channels A, B, C, D)
- Cue / PFL buttons (Ch A, B, C, D)
- Filter On buttons (Ch A, B, C, D)
- FX Assign buttons (FX 1 and FX 2 across channels C, A, B, D)
- Master Tempo rotary encoder and Tempo button
- Snap and Quantize buttons

---

## USB Interfaces & Endpoints

| Interface | Name                   | Number | Endpoint In        | Endpoint Out       | Type                             |
| --------- | ---------------------- | ------ | ------------------ | ------------------ | -------------------------------- |
| Control   | Traktor Kontrol S5 HID | 3      | `0x83` (interrupt) | `0x02` (interrupt) | Buttons, sliders, encoders, LEDs |
| Screen    | Traktor Kontrol S5 BD  | 4      | —                  | `0x03` (bulk)      | Dual screen pixel data           |

---

## Input Packets

### Buttons Packet (30 bytes)
Sent on interrupt endpoint `0x83`. Reports state changes for all transport, pad, mode, mixer, and touch detection switches.

### Sliders Packet (79 bytes)
Sent on interrupt endpoint `0x83`. Reports 12-bit normalized values for channel faders, crossfader, EQ/gain/filter knobs, FX dials, and relative fine positions for display encoders.

---

## Screen Blit Protocol

Both displays receive pixel data over bulk endpoint `0x03` on interface 4.

- **Format:** BGR565, big-endian (261,120 bytes per frame)
- **Resolution:** 480 × 272
- **Screen Selection:**
  - **Left Screen:** Header byte[2] = `0x00`, Footer byte[6] = `0x00`
  - **Right Screen:** Header byte[2] = `0x01`, Footer byte[6] = `0x01`

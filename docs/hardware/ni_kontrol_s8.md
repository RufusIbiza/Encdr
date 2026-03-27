# NI Kontrol S8 Hardware Reference

## Overview

| Property | Value |
|----------|-------|
| Manufacturer | Native Instruments |
| Product | Kontrol S8 |
| VID:PID | `0x17cc:0x1370` |
| USB Speed | High Speed (480 Mbps) |
| Interfaces | 3 (control + 2 x screen) |
| Screens | 2 x 480 x 272, BGR565 big-endian |
| Total LEDs | 309 |

The Kontrol S8 is a flagship 4-channel standalone mixer and DJ controller. It features two high-resolution displays, touch-sensitive knobs and faders, and a comprehensive mixer section.

---

## Physical Layout

The S8 is effectively two D2-style deck controllers flanking a central 4-channel mixer.

### Deck Sections (Left & Right)
- 4.3" Color Display (480x272)
- 4 Screen-adjacent buttons on each side of the screen
- 4 Touch-sensitive screen encoders below the screen
- 4 Performance faders with touch detection
- 4 FX knobs with touch detection
- 8 RGB performance pads
- 25-LED Touchstrip
- Browse and Loop push-encoders

### Mixer Section (Center)
- 4 Channel strips (A, B, C, D)
- Gain, 3-band EQ, and Filter knobs for each channel
- 4 Channel faders (touch-sensitive)
- Crossfader
- Master and Cue volume controls
- Master Tempo Encoder
- FX assign buttons for each channel
- Level meters for each channel and master output

---

## USB Interfaces

| Interface | ID | Number | Endpoint In | Endpoint Out | Type |
|-----------|----|--------|-------------|--------------|------|
| Control | `control` | 0 | `0x81` (interrupt) | `0x01` (interrupt) | Buttons, sliders, encoders, LEDs |
| Left Screen | `screen_left` | 1 | — | `0x02` (bulk) | Screen pixel data (Left) |
| Right Screen | `screen_right` | 2 | — | `0x03` (bulk) | Screen pixel data (Right) |

---

## Input Controls

The S8 uses larger packets than the D2 to accommodate the mixer section.

### Buttons Packet (46 bytes)
Sent on the control interface interrupt endpoint.
- Bytes 1-4: Deck buttons
- Byte 12: Mixer cue buttons (Masks: 0x01=A, 0x02=B, 0x04=C, 0x08=D)
- Byte 15: Transport buttons (Play, Cue, Sync, Shift)

### Sliders Packet (176 bytes)
Sent on the control interface when high-resolution controls (faders, EQs, touch sensors) are moved.
- Bytes 9-16: Channel faders (12-bit)
- Bytes 17-18: Crossfader (12-bit)
- Bytes 25-32: Mixer channel A (Gain, EQ High, Mid, Low)

---

## LED Output

The S8 has 309 LEDs. The output buffer is split into three segments using prefixes `0x80`, `0x81`, and `0x82`.

### LED Mapping
- **Pads (Left):** RGB LEDs 0-23 (3 bytes per pad)
- **FX Select (Left):** Index 24
- **Touchstrip (Left):** Indices 93-117 (25 LEDs)
- **Mixer Cue A-D:** Indices 25, 26, 57, 55 (relative to buffer start)
- **Channel Level Meters:** 
  - Channel A: Index 27
  - Channel B: Index 33
  - Channel C: Index 112
  - Channel D: Index 82

---

## Screen Blit Protocol

The S8 uses the same `0x84`/`0x40` protocol as the D2 and S5 for both screens.

Header: `84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`
Footer: `03 00 00 00 40 00 00 00`

Updates for the left screen are sent to interface 1 (endpoint `0x02`), and updates for the right screen are sent to interface 2 (endpoint `0x03`).

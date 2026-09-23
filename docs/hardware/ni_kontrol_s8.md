# NI Kontrol S8 Hardware Reference

## Overview

| Property     | Value                                                |
| ------------ | ---------------------------------------------------- |
| Manufacturer | Native Instruments                                   |
| Product      | Kontrol S8                                           |
| VID:PID      | `0x17cc:0x1370`                                      |
| USB Speed    | High Speed (480 Mbps)                                |
| Interfaces   | 7 (audio, MIDI, DFU, HID control, bulk display)      |
| Screens      | 2 x 480 x 272, BGR565 big-endian                     |
| Total LEDs   | 309-byte output buffer, split across 3 prefix groups |

The Kontrol S8 is a flagship 4-channel standalone mixer and DJ controller. It features two high-resolution displays, touch-sensitive knobs and faders, and a comprehensive mixer section.

---

## Physical Layout

The S8 is effectively two D2-style deck controllers flanking a central 4-channel mixer.

### Deck Sections (Left & Right)
- 4.3" Color Display (480x272)
- 8 Screen-adjacent buttons (4 left, 4 right of screen)
- 4 Performance knobs with touch detection
- 4 Performance faders with touch detection
- 4 FX knobs with touch detection
- 4 FX buttons + FX select
- 4 ON buttons
- 8 RGB performance pads
- 25-LED Touchstrip (bi-color: blue + orange)
- Transport: Play, Cue, Sync, Shift, Flux
- Mode: Hotcue, Loop, Freeze, Remix, Deck
- Navigation: Back, Capture, Edit

### Mixer Section (Center)
- 4 Channel faders (touch-sensitive)
- Crossfader
- 20 Analog knobs (Gain, Hi EQ, Mid EQ, Low EQ, Filter across channels A, B, C, D)
- Cue / PFL buttons (A, B, C, D)
- Filter On buttons (A, B, C, D)
- FX assign buttons (2 per channel: FX1 and FX2 for C, A, B, D)
- Snap, Quantize
- Master Tempo rotary encoder and Tempo button
- Crossfader Assign switches (Left/Right per channel)
- Mic 1, Mic 2

---

## USB Interfaces

| Interface | Name                      | Number | Endpoint In          | Endpoint Out         | Type                             |
| --------- | ------------------------- | ------ | -------------------- | -------------------- | -------------------------------- |
| Audio Out | Traktor Kontrol S8 Output | 1      | —                    | `0x01` (isochronous) | Audio output                     |
| Audio In  | Traktor Kontrol S8 Input  | 2      | `0x82` (isochronous) | —                    | Audio input                      |
| MIDI      | Traktor Kontrol S8 MIDI   | 3      | `0x83` (bulk)        | `0x02` (bulk)        | MIDI I/O                         |
| DFU       | Traktor Kontrol S8 DFU    | 4      | —                    | —                    | Firmware update                  |
| Control   | Traktor Kontrol S8 HID    | 5      | `0x84` (interrupt)   | `0x03` (interrupt)   | Buttons, sliders, encoders, LEDs |
| Screen    | Traktor Kontrol S8 BD     | 6      | —                    | `0x04` (bulk)        | Screen pixel data (both screens) |

---

## Input Controls

The S8 uses larger packets than the D2 to accommodate the mixer section and dual decks.

### Buttons Packet (41 bytes, Report ID 1)
Sent on the control interface (`0x84` interrupt in). Contains all button states for both decks and the mixer:
- Deck buttons: Play, Cue, Sync, Shift, Flux, Deck, Hotcue, Loop, Freeze, Remix, FX buttons, screen buttons, navigation buttons
- Mixer buttons: Cue/PFL (A, B, C, D), Filter On (A, B, C, D), FX Assign 1 & 2 (A, B, C, D), Snap, Quantize, Deck Assign, Tempo button, Mic 1 & 2
- Mixer encoders: Master Tempo rotary encoder (Report 1 byte 3, 4-bit `wrap16` counter)
- Crossfader assign: 8-switch matrix in byte 24 (`xfader_assign_left_*` and `xfader_assign_right_*`)

### Sliders Packet (109 bytes, Report ID 2)
Sent on the control interface when any analog control changes. Normalized to 0.0–1.0:
- **Per deck:** 4 performance knobs, 4 faders (touch-sensitive), 4 FX knobs (touch-sensitive), 4 screen encoders
- **Mixer Faders:** 4 channel faders (`mixer_fader_a`..`d`, 12-bit) and `crossfader` (12-bit)
- **Mixer Knobs (20 controls):**
  - Channel A: `mixer_gain_a` [69, 70], `mixer_eq_hi_a` [71, 72], `mixer_eq_mid_a` [73, 74], `mixer_eq_low_a` [75, 76], `mixer_filter_a` [77, 78] (12-bit)
  - Channel B: `mixer_gain_b` [79, 80], `mixer_eq_hi_b` [81, 82], `mixer_eq_mid_b` [83, 84], `mixer_eq_low_b` [85, 86], `mixer_filter_b` [87, 88] (12-bit)
  - Channel C: `mixer_gain_c` [89, 90], `mixer_eq_hi_c` [91, 92], `mixer_eq_mid_c` [93, 94], `mixer_eq_low_c` [96] (4-bit, max 15), `mixer_filter_c` [97, 98] (12-bit)
  - Channel D: `mixer_gain_d` [99, 100], `mixer_eq_hi_d` [101, 102], `mixer_eq_mid_d` [103, 104], `mixer_eq_low_d` [106] (4-bit, max 15), `mixer_filter_d` [107, 108] (12-bit)

### Control Naming Convention

All deck controls are prefixed with `left_` or `right_`.
Mixer controls use `mixer_` prefix: `mixer_fader_a`, `mixer_cue_a`, `mixer_gain_a`, `mixer_eq_hi_a`, `mixer_tempo`, `crossfader`, etc.

---

## LED Output

The S8 has two distinct LED output pathways:

### 1. Interrupt OUT Buffer (Endpoint `0x03`, Interface 5)

Split into three prefix groups sent via interrupt out:

| Prefix | ID           | Buffer Size | Description                                           |
| ------ | ------------ | ----------- | ----------------------------------------------------- |
| `0x80` | `left_deck`  | 118 bytes   | Left deck LEDs (pads, buttons, touchstrip)            |
| `0x81` | `right_deck` | 118 bytes   | Right deck LEDs (pads, buttons, touchstrip)           |
| `0x82` | `mixer`      | 73 bytes    | FX Assign 1 & 2 (C, A, B, D), Snap, Quantize (44–53)  |

### 2. Feature Report / Control Transfer LEDs (`0xF4`, EP0 Interface 5)

Due to the S8's standalone mixer architecture, Cue/PFL, Filter ON, and Direct Thru LEDs do not use the interrupt OUT buffer. Instead, they are sent via EP0 USB Control Transfer (`SET_REPORT`, `wValue=0x03F4`, `wIndex=5`):
- `[0xF4, 0x26, SS, ...]` — CUE / PFL LEDs (Ch A=0x01, B=0x02, C=0x04, D=0x08)
- `[0xF4, 0x25, SS, ...]` — FILTER ON LEDs (Ch A=0x01, B=0x02, C=0x04, D=0x08)
- `[0xF4, 0x24, SS, ...]` — DIRECT THRU LEDs (Ch A=0x01, B=0x02, C=0x04, D=0x08)

Encdr transparently handles this through the `quirks.feature_report_leds` configuration in the descriptor. Calling `set_led("mixer_cue_a", LedValue::Single(127))` automatically dispatches the proper control transfer without requiring caller awareness.

### Per-Deck LED Layout (identical for left/right)

| LED                     | Type   | Offset(s) | Notes                                 |
| ----------------------- | ------ | --------- | ------------------------------------- |
| Pad 1–8                 | RGB    | 0–23      | 3 bytes per pad: R, G, B (sequential) |
| FX Select               | Single | 24        |                                       |
| FX 1–4                  | Single | 25–28     |                                       |
| Screen Left 1–4         | Single | 29–32     |                                       |
| Screen Right 1–4        | Single | 33–36     |                                       |
| Back                    | Single | 37        |                                       |
| Capture                 | Single | 38        |                                       |
| Edit                    | Single | 39        |                                       |
| ON 1–4                  | Single | 40–43     |                                       |
| Hotcue (white/blue)     | Dual   | 44–45     |                                       |
| Loop (white/blue)       | Dual   | 46–47     |                                       |
| Freeze (white/blue)     | Dual   | 48–49     |                                       |
| Remix (white/blue)      | Dual   | 50–51     |                                       |
| Flux                    | Single | 52        |                                       |
| Deck (white/blue)       | Dual   | 53–54     |                                       |
| Shift                   | Single | 55        |                                       |
| Sync (green/red)        | Dual   | 56–57     |                                       |
| Cue                     | Single | 58        |                                       |
| Play                    | Single | 59        |                                       |
| Loop Circle 1–4 (white) | Single | 60–63     |                                       |
| Loop Circle 1–4 (blue)  | Single | 64–67     |                                       |
| Touchstrip Blue         | Strip  | 68–92     | 25 LEDs                               |
| Touchstrip Orange       | Strip  | 93–117    | 25 LEDs                               |
| Deck A/B/C/D            | Single | 118–121   | Deck selector indicators              |

**Note:** RGB pad ordering on S8 is R=0, G=1, B=2 (sequential), unlike the D2 which uses B=0, G=1, R=2 (inverted).

---

## Screen Blit Protocol

Both screens share a single bulk endpoint (`0x04` on interface 6). The S8 uses the NI bulk blit protocol with screen selection in the header and footer.

### Full Blit Format

```
[20-byte header] [261,120 bytes pixel data] [8-byte footer]
```

### Screen Selection

The target screen is identified by **byte[3]** of the header and **byte[6]** of the footer:

| Screen | Header byte[3] | Footer byte[6] | Full Header                                                   |
| ------ | -------------- | -------------- | ------------------------------------------------------------- |
| Left   | `0x60`         | `0x00`         | `84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00` |
| Right  | `0x60`         | `0x01`         | `84 00 01 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00` |

| Screen | Footer                    |
| ------ | ------------------------- |
| Left   | `03 00 00 00 40 00 00 00` |
| Right  | `03 00 00 00 40 00 01 00` |

### Pixel Format

- **Format:** BGR565, big-endian
- **Resolution:** 480 x 272
- **Size:** 480 x 272 x 2 = 261,120 bytes per frame

### Key Differences from D2/Mk3

1. **Header byte[3]:** S8 uses `0x60` for both screens (same as D2). The Mk3 also uses `0x60`.
2. **Header byte[2]:** `0x00` for left, `0x01` for right (same as Mk3).
3. **Footer:** The right screen footer has byte[6] = `0x01`. This is unique to controllers with dual screens on a shared endpoint (confirmed from S5 Ctlra implementation).
4. **Single endpoint:** Both screens use interface 6, endpoint `0x04` (bulk out). The D2 has only one screen.

---

## Quirks

- `dual_handle`: The S8 requires claiming multiple USB interfaces simultaneously (control + screen).
- `detach_kernel_driver`: The kernel driver must be detached before claiming interfaces.
- `feature_report_leds`: Manages EP0 Control Transfer Feature Report `0xF4` for mixer Cue/PFL, Filter On, and Direct Thru LEDs.
- Interfaces 1 and 2 are audio (isochronous), not screens — despite some documentation suggesting otherwise.
- The MIDI interface (3) has a bulk out endpoint (`0x02`) that should not be confused with screen data.

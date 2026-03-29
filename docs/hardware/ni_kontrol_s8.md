# NI Kontrol S8 Hardware Reference

## Overview

| Property | Value |
|----------|-------|
| Manufacturer | Native Instruments |
| Product | Kontrol S8 |
| VID:PID | `0x17cc:0x1370` |
| USB Speed | High Speed (480 Mbps) |
| Interfaces | 7 (audio, MIDI, DFU, HID control, bulk display) |
| Screens | 2 x 480 x 272, BGR565 big-endian |
| Total LEDs | 309-byte output buffer, split across 3 prefix groups |

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
- Cue buttons (A, B, C, D)
- Filter On buttons (A, B, C, D)
- FX assign buttons (2 per channel)
- Snap, Quantize
- Mic 1, Mic 2

---

## USB Interfaces

| Interface | Name | Number | Endpoint In | Endpoint Out | Type |
|-----------|------|--------|-------------|--------------|------|
| Audio Out | Traktor Kontrol S8 Output | 1 | — | `0x01` (isochronous) | Audio output |
| Audio In | Traktor Kontrol S8 Input | 2 | `0x82` (isochronous) | — | Audio input |
| MIDI | Traktor Kontrol S8 MIDI | 3 | `0x83` (bulk) | `0x02` (bulk) | MIDI I/O |
| DFU | Traktor Kontrol S8 DFU | 4 | — | — | Firmware update |
| Control | Traktor Kontrol S8 HID | 5 | `0x84` (interrupt) | `0x03` (interrupt) | Buttons, sliders, encoders, LEDs |
| Screen | Traktor Kontrol S8 BD | 6 | — | `0x04` (bulk) | Screen pixel data (both screens) |

---

## Input Controls

The S8 uses larger packets than the D2 to accommodate the mixer section and dual decks.

### Buttons Packet (41 bytes)
Sent on the control interface (`0x84` interrupt in). Contains all button states for both decks and the mixer.

### Sliders Packet (176 bytes)
Sent on the control interface when any analog control changes. All 12-bit values, normalized to 0.0–1.0.

**Per deck:** 4 performance knobs (touch-sensitive), 4 faders (touch-sensitive), 4 FX knobs (touch-sensitive)
**Mixer:** 4 channel faders, crossfader

### Control Naming Convention

All deck controls are prefixed with `left_` or `right_`:
- `left_fx_knob_1` through `left_fx_knob_4`
- `left_perf_knob_1` through `left_perf_knob_4`
- `left_fader_1` through `left_fader_4`
- `left_fx_button_1`, `left_play`, `left_cue`, etc.

Mixer controls use `mixer_` prefix: `mixer_fader_a`, `mixer_cue_a`, `crossfader`, etc.

---

## LED Output

The S8 LED output buffer is 309 bytes, split into three prefix groups sent via interrupt out on endpoint `0x03` (interface 5).

### Prefix Groups

| Prefix | ID | Description |
|--------|----|-------------|
| `0x80` | `left_deck` | Left deck LEDs (pads, buttons, touchstrip) |
| `0x81` | `right_deck` | Right deck LEDs (pads, buttons, touchstrip) |
| `0x82` | `mixer` | Mixer LEDs (cue buttons, meters) |

### Per-Deck LED Layout (identical for left/right)

| LED | Type | Offset(s) | Notes |
|-----|------|-----------|-------|
| Pad 1–8 | RGB | 0–23 | 3 bytes per pad: R, G, B (sequential) |
| FX Select | Single | 24 | |
| FX 1–4 | Single | 25–28 | |
| Screen Left 1–4 | Single | 29–32 | |
| Screen Right 1–4 | Single | 33–36 | |
| Back | Single | 37 | |
| Capture | Single | 38 | |
| Edit | Single | 39 | |
| ON 1–4 | Single | 40–43 | |
| Hotcue (white/blue) | Dual | 44–45 | |
| Loop (white/blue) | Dual | 46–47 | |
| Freeze (white/blue) | Dual | 48–49 | |
| Remix (white/blue) | Dual | 50–51 | |
| Flux | Single | 52 | |
| Deck (white/blue) | Dual | 53–54 | |
| Shift | Single | 55 | |
| Sync (green/red) | Dual | 56–57 | |
| Cue | Single | 58 | |
| Play | Single | 59 | |
| Loop Circle 1–4 (white) | Single | 60–63 | |
| Loop Circle 1–4 (blue) | Single | 64–67 | |
| Touchstrip Blue | Strip | 68–92 | 25 LEDs |
| Touchstrip Orange | Strip | 93–117 | 25 LEDs |
| Deck A/B/C/D | Single | 118–121 | Deck selector indicators |

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

| Screen | Header byte[3] | Footer byte[6] | Full Header |
|--------|---------------|----------------|-------------|
| Left   | `0x60`        | `0x00`         | `84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00` |
| Right  | `0x60`        | `0x01`         | `84 00 01 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00` |

| Screen | Footer |
|--------|--------|
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
- Interfaces 1 and 2 are audio (isochronous), not screens — despite some documentation suggesting otherwise.
- The MIDI interface (3) has a bulk out endpoint (`0x02`) that should not be confused with screen data.

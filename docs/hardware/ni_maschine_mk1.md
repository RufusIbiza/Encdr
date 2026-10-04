# NI Maschine Mk1 Hardware Reference

## Overview

| Property     | Value                                                         |
| ------------ | ------------------------------------------------------------- |
| Manufacturer | Native Instruments                                            |
| Product      | Maschine Controller (Maschine Mk1)                            |
| VID:PID      | `0x17cc:0x0808`                                               |
| USB Speed    | High Speed (480 Mbps)                                         |
| Interfaces   | 1 vendor-specific interface (class `ff/ff/00`), alt setting 1 |
| Screens      | 2x 255 x 64 grayscale LCD (Sitronix ST7529, 32 levels)        |

The original Maschine (2009) has two grayscale LCDs, 8 display buttons above them, 11 endless knobs (8 under the screens plus Volume, Tempo and Swing), 16 pressure-sensitive pads, 8 group buttons (A–H) and transport/mode buttons. Every LED, pads included, is single-color with adjustable brightness.

Unlike the Mk2 and later, the Mk1 is not a HID device. It speaks the older NI "caiaq" command protocol over bulk endpoints, the same family as the Kore controllers and Audio 8 DJ.

Every mapping on this page was verified against physical hardware.

---

## Getting Started

### Linux Requirements

The kernel's `snd-usb-caiaq` driver binds to the Mk1 and exposes it as an ALSA card plus an input device. Encdr detaches it automatically when it claims the interface.

1. **Udev Rules (Required):** Create or update `/etc/udev/rules.d/99-ni-controllers.rules`:
   ```bash
   SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"
   ```
2. **Reload Rules:**
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

---

## Physical Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│  [Control] [Step]    [top_1][top_2][top_3][top_4] [top_5][top_6][top_7][top_8]
│  [Browse ] [Sampling]┌──────────────────────┐ ┌──────────────────────┐   │
│  [◄]  [►]            │     LEFT SCREEN      │ │     RIGHT SCREEN     │ (Volume)
│  [Snap] [Auto Write] │      255 x 64        │ │      255 x 64        │ (Tempo)
│                      └──────────────────────┘ └──────────────────────┘ (Swing)
│                       (enc1)(enc2)(enc3)(enc4) (enc5)(enc6)(enc7)(enc8)  │
│                                                         [Note Repeat]    │
│  [Grp A] [Grp B] [Grp C] [Grp D]   [Scene]   [Pattern]  [Pad Mode]       │
│  [Grp E] [Grp F] [Grp G] [Grp H]   [Navigate][Duplicate][Select]         │
│                                    [Solo]    [Mute]                      │
│  [Restart] [◄] [►] [Grid]          [Pad13] [Pad14] [Pad15] [Pad16]       │
│  [Play] [Rec] [Erase] [Shift]      [Pad 9] [Pad10] [Pad11] [Pad12]       │
│                                    [Pad 5] [Pad 6] [Pad 7] [Pad 8]       │
│                                    [Pad 1] [Pad 2] [Pad 3] [Pad 4]       │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## USB Endpoints

All endpoints live on interface 0 and only appear after selecting **alternate setting 1**. Every endpoint is **bulk** with a 512-byte max packet size.

| Endpoint | Dir | Descriptor interface id | Purpose                                         |
| -------- | --- | ----------------------- | ----------------------------------------------- |
| `0x01`   | OUT | `control`               | Commands: LEDs (`0x0c`), auto-report setup (`0x0b`) |
| `0x81`   | IN  | `control`               | Command replies, button (`0x04`) and knob (`0x02`) reports |
| `0x84`   | IN  | `pads`                  | Continuous pad pressure stream                  |
| `0x08`   | OUT | `display`               | Both LCDs                                       |

**Command back-pressure:** the device holds each EP1 command until the host reads the previous reply from `0x81`. If nothing drains `0x81`, a later command such as an LED write times out. Encdr queues its reads before sending anything, so this never stalls.

### Initialization

On connect, Encdr does the following:

1. Selects alt setting 1.
2. Queues reads on `0x81` and `0x84`.
3. Sends `AUTO_MSG` (`0b 01 0a 05`), which enables spontaneous button and knob reports.
4. Runs the ST7529 init sequence on each display (see below).
5. Turns the display backlight on.

---

## Input Reports

### Buttons — `0x04` on `0x81` (8 bytes)

| Byte | `0x01` | `0x02` | `0x04` | `0x08` | `0x10` | `0x20` | `0x40` | `0x80` |
| ---- | ------ | ------ | ------ | ------ | ------ | ------ | ------ | ------ |
| 1 | `mute` | `solo` | `select` | `duplicate` | `navigate` | `pad_mode` | `pattern` | `scene` |
| 2 | — | `rec` | `erase` | `shift` | `grid` | `step_right` | `step_left` | `restart` |
| 3 | `group_e` | `group_f` | `group_g` | `group_h` | `group_d` | `group_c` | `group_b` | `group_a` |
| 4 | `control` | `browser` | `arrow_left` | `snap` | `auto_write` | `arrow_right` | `sampling` | `step` |
| 5 | `top_8` | `top_7` | `top_6` | `top_5` | `top_4` | `top_3` | `top_2` | `top_1` |
| 6 | `note_repeat` | `play` | — | — | — | — | *(toggle)* | *(toggle)* |

Bits `0x40`/`0x80` of byte 6 swap on every report and byte 7 is a constant `0x85`. They carry no control state and are ignored.

`step_left`/`step_right` are the transport `◄`/`►` buttons. `arrow_left`/`arrow_right` are the browse `◄`/`►` buttons.

### Knobs — `0x02` on `0x81` (33 bytes)

The 11 knobs are **endless rotary potentiometers (ERP)**, not digital encoders. Each reports two 8-bit analog wiper taps 90° apart, at bytes `[b, a] = [1 + 2i, 2 + 2i]`. Encdr decodes each pair to an absolute 0–999 position per turn, using the `decode_erp` calibration from the Linux `snd-usb-caiaq` driver. It then emits relative `EncoderFine` deltas, in fractions of a full turn (`scale: 1000`).

| Pair | Bytes | Name |
| ---- | ----- | ---- |
| 0 | 1, 2 | `screen_encoder_8` |
| 1 | 3, 4 | `screen_encoder_4` |
| 2 | 5, 6 | `swing` |
| 3 | 7, 8 | `screen_encoder_7` |
| 4 | 9, 10 | `screen_encoder_3` |
| 5 | 11, 12 | `tempo` |
| 6 | 13, 14 | `screen_encoder_6` |
| 7 | 15, 16 | `screen_encoder_2` |
| 8 | 17, 18 | `volume` |
| 9 | 19, 20 | `screen_encoder_5` |
| 10 | 21, 22 | `screen_encoder_1` |

Clockwise rotation produces positive deltas.

**Filtering:**
- **Idle jitter.** The firmware sends a report only when a knob moves, but a resting knob can still wander ±5–7 thousandths of a turn. A deadband of 10 (1% of a turn) absorbs this. Movement smaller than the deadband is held back until it accumulates, so it is never lost.
- **Crosstalk.** Spinning one knob fast can briefly nudge a neighbor by about ±0.02 turns, in a +/− pair that nets to roughly zero.

### Pads — stream on `0x84`

Pads arrive as a continuous stream of little-endian 16-bit words:

```
bits 15..12  hardware pad index (0–15)
bits 11..0   pressure (0–4095)
```

- **Word order.** Each word names its own pad, and frames of 16 words are **not aligned** to transfer boundaries (a transfer may start mid-cycle). The descriptor marks the stream `"pad_format": "id_pressure_words"` so it is decoded word by word.
- **Thresholds.** Resting pads read exactly 0, light hits peak above 2000, and hard hits reach about 3600. A pad presses at a raw value of 256 and releases below 128.
- **Index mapping.** Hardware index *n* maps to pads in the same order as the Mk3: index 0 is `pad_13` (top-left) and index 12 is `pad_1` (bottom-left). `Event::Grid::index` carries the hardware index.

---

## LED Output

LEDs are written with the `DIMM_LEDS` command `0x0c` on EP `0x01`, in two banks of 31 brightness bytes:

- `bank_a`: `0c 00` + 31 bytes
- `bank_b`: `0c 1e` + 31 bytes

All LEDs are single-color. NI's own software drives them at up to `0x5c` (92), and Encdr's examples use 0–92.

**LED writes are expensive.** Each bank write ties up the controller for about 5 ms, and the display stream pauses meanwhile. Rewriting both banks 33 times a second cuts full-frame screen throughput by about 30%; at 100 times a second, by half. Update LEDs only when their values change; continuously animating every LED costs display time.

| Offset | `bank_a` | `bank_b` |
| ------ | -------- | -------- |
| 0–3 | `pad_4`, `pad_3`, `pad_2`, `pad_1` | `step_left`, `restart`, `group_h`, `group_g` |
| 4–7 | `pad_8`, `pad_7`, `pad_6`, `pad_5` | `group_d`, `group_c`, `group_f`, `group_e` |
| 8–11 | `pad_12`, `pad_11`, `pad_10`, `pad_9` | `group_b`, `group_a`, `auto_write`, `snap` |
| 12–15 | `pad_16`, `pad_15`, `pad_14`, `pad_13` | `arrow_right`, `arrow_left`, `sampling`, `browser` |
| 16–19 | `mute`, `solo`, `select`, `duplicate` | `step`, `control`, `top_8`, `top_7` |
| 20–23 | `navigate`, `pad_mode`, `pattern`, `scene` | `top_6`, `top_5`, `top_4`, `top_3` |
| 24–27 | `shift`, `erase`, `grid`, `step_right` | `top_2`, `top_1`, `note_repeat`, `display_backlight` |
| 28–30 | `rec`, `play`, — | — |

`display_backlight` defaults to 92 on connect (via the descriptor's `"default"`). Without it the LCDs are unreadable. Setting it to 0 turns the backlight off.

---

## Screen Protocol

Both displays share EP `0x08` and are Sitronix ST7529 controllers behind NI's USB bridge. Every transfer is framed as:

```
[header, len_hi, len_lo, payload…]      len = payload length (big-endian)
header = display << 1        payload starts with an ST7529 command
header = display << 1 | 1    payload is a data continuation
```

`display` is 0 for the left screen and 1 for the right.

**Init** runs the 22-command ST7529 sequence from [shaduzlabs/cabl](https://github.com/shaduzlabs/cabl) once per display, with 20 ms settling delays. It ends in 3-pixel/2-byte data mode.

**Frame** (10,880 bytes per screen):

1. `[h, 00, 03, 75, 00, 3f]` — LASET, lines 0–63
2. `[h, 00, 03, 15, 00, 54]` — CASET, columns 0–84 (3 pixels each)
3. `[h, 01, f7, 5c]` + 502 bytes — RAMWR plus the first data chunk
4. 20 × `[h|1, 01, f6]` + 502 bytes
5. `[h|1, 01, 52]` + 338 bytes

**Pixel format `st7529_gray5`:** each 255-pixel row packs into 85 groups of 3 pixels in 2 bytes, for 170 bytes per row:

```
byte 0: [p0 4:0][p1 4:2]
byte 1: [p1 1:0][unused][p2 4:0]
```

Levels are 5-bit and **inverted**: 0 is fully lit and 31 is black. Submit RGBA8888 or RGB888 frames and Encdr converts luminance for you.

**Throughput.** All measurements are on hardware.

| | Data rate | Time | Notes |
| --- | --- | --- | --- |
| Display bridge | ~310 KiB/s | — | Transfer time scales linearly with bytes |
| Full frame | — | ~34 ms | About 29 full frames/s, shared by both screens |

- Encdr sends each changed frame in full and skips frames identical to the last one sent, so a static screen costs nothing.
- Transfers carrying more than 502 data bytes are far slower (hundreds of ms per frame), so frames are always chunked at 502.

---

## Running the Example

```bash
cargo run -p encdr-examples --bin mk1_vegas
```

This animates every LED and draws live knob-position bars and a pad pressure grid on both screens, while printing every button, pad and knob event.

---

## References

- Linux kernel `sound/usb/caiaq` (`device.c`, `input.c`, `control.c`): endpoints, button bitmap, ERP decoding, pad words, LED banks.
- [shaduzlabs/cabl](https://github.com/shaduzlabs/cabl) `MaschineMK1.cpp` / `GDisplayMaschineMK1.cpp`: display init, framing and pixel packing.

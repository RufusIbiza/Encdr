# NI Kontrol D2 Hardware Reference

## Overview

| Property | Value |
|----------|-------|
| Manufacturer | Native Instruments |
| Product | Kontrol D2 |
| VID:PID | `0x17cc:0x1400` |
| USB Speed | High Speed (480 Mbps) |
| Interfaces | 2 (control + screen) |
| Screen | 480 x 272, BGR565 big-endian |

The Kontrol D2 is a deck controller with a 4.3" color display, four touch-sensitive screen encoders, four faders with touch detection, four FX knobs, eight RGB performance pads, a touchstrip, and browse/loop push-encoders.

---

## Physical Layout

```
┌─────────────────────────────────────────────────────────┐
│  [FX Select]                                            │
│                                                         │
│  [FX 1]  [FX 2]  [FX 3]  [FX 4]    ← FX Buttons       │
│  (dial)  (dial)  (dial)  (dial)     ← FX Dials (touch)  │
│                                                         │
│  [screen_left_1]  ┌──────────────┐  [screen_right_1]   │
│  [screen_left_2]  │              │  [screen_right_2]   │
│  [screen_left_3]  │    480x272   │  [screen_right_3]   │
│  [screen_left_4]  │    SCREEN    │  [screen_right_4]   │
│                   │              │                      │
│      (enc 1) (enc 2) (enc 3) (enc 4)  ← Screen Encoders│
│                   └──────────────┘                      │
│                                                         │
│  [Back] [Capture] [Edit]  (Browse)  (Loop) ← Encoders  │
│                                                         │
│  [ON 1]  [ON 2]  [ON 3]  [ON 4]                       │
│  |fdr1|  |fdr2|  |fdr3|  |fdr4|    ← Faders (touch)   │
│                                                         │
│  [Pad1] [Pad2] [Pad3] [Pad4]       ← Top row          │
│  [Pad5] [Pad6] [Pad7] [Pad8]       ← Bottom row       │
│                                                         │
│  [HotCue] [Loop] [Freeze] [Remix]                      │
│  [Flux]   [Deck]                                        │
│                                                         │
│  ═══════ TOUCHSTRIP ═══════                             │
│                                                         │
│  [A] [B] [C] [D]                   ← Deck Selectors    │
│  [Shift] [Sync] [Cue] [Play]       ← Transport         │
└─────────────────────────────────────────────────────────┘
```

---

## USB Interfaces

| Interface | ID | Number | Endpoint In | Endpoint Out | Type |
|-----------|----|--------|-------------|--------------|------|
| Control | `control` | 0 | `0x81` (interrupt) | `0x01` (interrupt) | Buttons, sliders, encoders, LEDs |
| Screen | `screen` | 1 | — | `0x02` (bulk) | Screen pixel data |

**Quirk: `dual_handle`** — The D2 requires separate USB interface handles (one per interface). Both are claimed from the same USB device.

**Quirk: `detach_kernel_driver`** — On Linux, the HID kernel driver must be detached before claiming the interface.

---

## Input Controls

The D2 sends two packet types, discriminated by size:

### Buttons Packet (17 bytes)

Sent on the control interface interrupt endpoint when any button, touch sensor, or encoder state changes.

#### Buttons (52 total)

| Name | Byte | Mask | Description |
|------|------|------|-------------|
| `deck_a` | 5 | `0x01` | Deck A selector |
| `deck_b` | 5 | `0x02` | Deck B selector |
| `deck_c` | 5 | `0x04` | Deck C selector |
| `deck_d` | 5 | `0x08` | Deck D selector |
| `fx_1` | 2 | `0x80` | FX slot 1 toggle |
| `fx_2` | 3 | `0x04` | FX slot 2 toggle |
| `fx_3` | 3 | `0x02` | FX slot 3 toggle |
| `fx_4` | 3 | `0x01` | FX slot 4 toggle |
| `fx_select` | 2 | `0x40` | FX select button |
| `screen_left_1` | 2 | `0x20` | Left screen button, top |
| `screen_left_2` | 2 | `0x10` | Left screen button, 2nd |
| `screen_left_3` | 2 | `0x01` | Left screen button, 3rd |
| `screen_left_4` | 4 | `0x40` | Left screen button, bottom |
| `screen_right_1` | 3 | `0x08` | Right screen button, top |
| `screen_right_2` | 3 | `0x10` | Right screen button, 2nd |
| `screen_right_3` | 3 | `0x20` | Right screen button, 3rd |
| `screen_right_4` | 3 | `0x40` | Right screen button, bottom |
| `encoder_browse_press` | 2 | `0x08` | Browse encoder push |
| `back` | 4 | `0x80` | Back button |
| `capture` | 4 | `0x20` | Capture button |
| `edit` | 4 | `0x01` | Edit button |
| `encoder_loop_press` | 6 | `0x10` | Loop encoder push |
| `on_1` | 4 | `0x10` | Fader 1 on/off |
| `on_2` | 4 | `0x08` | Fader 2 on/off |
| `on_3` | 4 | `0x04` | Fader 3 on/off |
| `on_4` | 4 | `0x02` | Fader 4 on/off |
| `pad_1` | 7 | `0x08` | Performance pad 1 (top-left) |
| `pad_2` | 7 | `0x01` | Performance pad 2 |
| `pad_3` | 6 | `0x01` | Performance pad 3 |
| `pad_4` | 6 | `0x02` | Performance pad 4 (top-right) |
| `pad_5` | 7 | `0x20` | Performance pad 5 (bottom-left) |
| `pad_6` | 7 | `0x40` | Performance pad 6 |
| `pad_7` | 7 | `0x80` | Performance pad 7 |
| `pad_8` | 7 | `0x02` | Performance pad 8 (bottom-right) |
| `hotcue` | 8 | `0x08` | Hot Cue mode |
| `loop` | 8 | `0x04` | Loop mode |
| `freeze` | 8 | `0x02` | Freeze mode |
| `remix` | 8 | `0x01` | Remix mode |
| `flux` | 7 | `0x10` | Flux button |
| `deck` | 7 | `0x04` | Deck button |
| `shift` | 8 | `0x80` | Shift modifier |
| `sync` | 8 | `0x40` | Sync button |
| `cue` | 8 | `0x20` | Cue button |
| `play` | 8 | `0x10` | Play button |

#### Touch Sensors (12 total)

Touch sensors emit `Event::Touch` with `touched: bool`.

| Name | Byte | Mask | Description |
|------|------|------|-------------|
| `fx_dial_touch_1` | 9 | `0x40` | FX dial 1 touch |
| `fx_dial_touch_2` | 9 | `0x80` | FX dial 2 touch |
| `fx_dial_touch_3` | 10 | `0x10` | FX dial 3 touch |
| `fx_dial_touch_4` | 10 | `0x20` | FX dial 4 touch |
| `screen_encoder_touch_1` | 9 | `0x02` | Screen encoder 1 touch |
| `screen_encoder_touch_2` | 9 | `0x04` | Screen encoder 2 touch |
| `screen_encoder_touch_3` | 9 | `0x08` | Screen encoder 3 touch |
| `screen_encoder_touch_4` | 9 | `0x10` | Screen encoder 4 touch |
| `encoder_browse_touch` | 9 | `0x20` | Browse encoder touch |
| `encoder_loop_touch` | 9 | `0x01` | Loop encoder touch |
| `fader_touch_1` | 10 | `0x01` | Fader 1 touch |
| `fader_touch_2` | 10 | `0x02` | Fader 2 touch |
| `fader_touch_3` | 10 | `0x04` | Fader 3 touch |
| `fader_touch_4` | 10 | `0x08` | Fader 4 touch |

#### Encoders (2)

Notched push-encoders with 4-bit wraparound counters. Emit `Event::Encoder` with `delta: i32` (typically +1 or -1).

| Name | Byte | Bits | Bit Offset | Encoding |
|------|------|------|------------|----------|
| `browse` | 1 | 4 | 4 | `wrap16` |
| `loop_enc` | 1 | 4 | 0 | `wrap16` |

#### Touchstrip (1 slider + 1 touch)

The touchstrip emits both a position slider and a touch event. Touch is detected as the 16-bit position value being non-zero.

| Name | Type | Bytes | Description |
|------|------|-------|-------------|
| `touchstrip_touch` | `Event::Touch` | [13, 14] | Multi-byte touch (value > 0) |
| `touchstrip` | `Event::Slider` | [13, 14] | Position, normalized 0.0-1.0 (max 1024) |

Emits `Event::Slider` with `value: f32` normalized 0.0-1.0.

### Sliders Packet (25 bytes)

Sent on the control interface when any fader, dial, or screen encoder value changes.

#### Screen Encoders (4)

Smooth, touch-sensitive encoders below the screen. Emit `Event::EncoderFine` with `delta: f32` (high resolution, scaled by 999.0).

| Name | Bytes | Encoding | Scale |
|------|-------|----------|-------|
| `screen_encoder_1` | [1, 2] | `signed16` | 999.0 |
| `screen_encoder_2` | [3, 4] | `signed16` | 999.0 |
| `screen_encoder_3` | [5, 6] | `signed16` | 999.0 |
| `screen_encoder_4` | [7, 8] | `signed16` | 999.0 |

#### Faders (4)

Touch-sensitive vertical faders. Emit `Event::Slider` with `value: f32` normalized 0.0-1.0.

| Name | Bytes | Bits | Max Value |
|------|-------|------|-----------|
| `fader_1` | [9, 10] | 12 | 4078 |
| `fader_2` | [11, 12] | 12 | 4078 |
| `fader_3` | [13, 14] | 12 | 4078 |
| `fader_4` | [15, 16] | 12 | 4078 |

#### FX Dials (4)

Touch-sensitive continuous rotary knobs. Emit `Event::Slider` with `value: f32` normalized 0.0-1.0.

| Name | Bytes | Bits | Max Value |
|------|-------|------|-----------|
| `fx_dial_1` | [17, 18] | 12 | 4078 |
| `fx_dial_2` | [19, 20] | 12 | 4078 |
| `fx_dial_3` | [21, 22] | 12 | 4078 |
| `fx_dial_4` | [23, 24] | 12 | 4078 |

---

## LED Output

The D2's LED buffer is 122 bytes, sent on the control interface interrupt out endpoint (`0x01`) with a `0x80` prefix byte.

### RGB Pad LEDs (8)

Each pad has an RGB LED with independent red, green, blue channels (0-255).

| Name | R Offset | G Offset | B Offset |
|------|----------|----------|----------|
| `pad_1` | 2 | 1 | 0 |
| `pad_2` | 5 | 4 | 3 |
| `pad_3` | 8 | 7 | 6 |
| `pad_4` | 11 | 10 | 9 |
| `pad_5` | 14 | 13 | 12 |
| `pad_6` | 17 | 16 | 15 |
| `pad_7` | 20 | 19 | 18 |
| `pad_8` | 23 | 22 | 21 |

Set with `LedValue::Rgb { r, g, b }`.

### Single-Color LEDs

All set with `LedValue::Single(brightness)` where brightness is 0-255.

#### FX Section (5)

| Name | Offset | Description |
|------|--------|-------------|
| `fx_select` | 24 | FX Select button |
| `fx_1` | 25 | FX 1 button |
| `fx_2` | 26 | FX 2 button |
| `fx_3` | 27 | FX 3 button |
| `fx_4` | 28 | FX 4 button |

#### Screen Buttons (8)

| Name | Offset | Description |
|------|--------|-------------|
| `screen_left_1` | 29 | Left of screen, top |
| `screen_left_2` | 30 | Left of screen, 2nd |
| `screen_left_3` | 31 | Left of screen, 3rd |
| `screen_left_4` | 32 | Left of screen, bottom |
| `screen_right_1` | 33 | Right of screen, top |
| `screen_right_2` | 34 | Right of screen, 2nd |
| `screen_right_3` | 35 | Right of screen, 3rd |
| `screen_right_4` | 36 | Right of screen, bottom |

#### Navigation (3)

| Name | Offset | Description |
|------|--------|-------------|
| `back` | 37 | Back button |
| `capture` | 38 | Capture button |
| `edit` | 39 | Edit button |

#### ON Buttons (4)

| Name | Offset | Description |
|------|--------|-------------|
| `on_1` | 40 | ON 1 (below screen encoder 1) |
| `on_2` | 41 | ON 2 (below screen encoder 2) |
| `on_3` | 42 | ON 3 (below screen encoder 3) |
| `on_4` | 43 | ON 4 (below screen encoder 4) |

#### Mode Buttons — Dual Color (5)

These buttons have two LED components (white/brightness + blue). Set each independently.

| White Name | Offset | Blue Name | Offset | Description |
|-----------|--------|-----------|--------|-------------|
| `hotcue_white` | 44 | `hotcue_blue` | 45 | Hotcue mode |
| `loop_white` | 46 | `loop_blue` | 47 | Loop mode |
| `freeze_white` | 48 | `freeze_blue` | 49 | Freeze mode |
| `remix_white` | 50 | `remix_blue` | 51 | Remix mode |
| `deck_white` | 53 | `deck_blue` | 54 | Deck button |

#### Transport & Other (5)

| Name | Offset | Description |
|------|--------|-------------|
| `flux` | 52 | Flux button |
| `shift` | 55 | Shift button |
| `sync_green` | 56 | Sync button green component |
| `sync_red` | 57 | Sync button red component |
| `cue` | 58 | Cue button |
| `play` | 59 | Play button |

#### Loop Circle LEDs — Dual Color (4)

Ring of indicator LEDs around the loop encoder. Each has white and blue components.

| White Name | Offset | Blue Name | Offset |
|-----------|--------|-----------|--------|
| `loop_circle_1_white` | 60 | `loop_circle_1_blue` | 64 |
| `loop_circle_2_white` | 61 | `loop_circle_2_blue` | 65 |
| `loop_circle_3_white` | 62 | `loop_circle_3_blue` | 66 |
| `loop_circle_4_white` | 63 | `loop_circle_4_blue` | 67 |

#### Deck Selectors (4)

| Name | Offset | Description |
|------|--------|-------------|
| `deck_a` | 118 | Deck A selector |
| `deck_b` | 119 | Deck B selector |
| `deck_c` | 120 | Deck C selector |
| `deck_d` | 121 | Deck D selector |

### Touchstrip LED Strips (2)

Two independent LED strips along the touchstrip, each with 25 individually addressable LEDs.

| Name | Offset | Count | Color |
|------|--------|-------|-------|
| `touchstrip_blue` | 68 | 25 | Blue |
| `touchstrip_orange` | 93 | 25 | Orange |

Set with `set_led_strip(device_id, name, &[u8; 25])`.

---

## Screen

| Property | Value |
|----------|-------|
| Resolution | 480 x 272 pixels |
| Pixel Format | BGR565 big-endian |
| Bytes per pixel | 2 |
| Full frame size | 261,120 bytes (480 * 272 * 2) |
| Interface | `screen` (interface 1) |
| Endpoint | `0x02` (bulk out) |
| Partial update support | Yes |
| Partial X alignment | 4 pixels |
| Partial Y alignment | 2 pixels |

### Full Blit Protocol

A full screen update is sent as:

```
[20-byte header] [261,120 bytes pixel data] [8-byte footer]
```

Header: `84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`
Footer: `03 00 00 00 40 00 00 00`

### Partial Blit Protocol

Partial updates send only a rectangular region:

```
[20-byte header with coordinates] [region pixel data] [8-byte footer]
```

The header template uses coordinate substitution:
- `{x_hi}`, `{x_lo}` — X offset (big-endian u16)
- `{y_hi}`, `{y_lo}` — Y offset
- `{w_hi}`, `{w_lo}` — Width
- `{h_hi}`, `{h_lo}` — Height
- `{px_half_hi}`, `{px_half_lo}` — Half the pixel count (region_w * region_h / 2)

Coordinates must be aligned to 4 pixels horizontally and 2 pixels vertically.

---

## Controls Grouped by Screen Proximity

For building screen UIs, here are the controls physically adjacent to the screen:

### Above the Screen

| Position | Button | Touch |
|----------|--------|-------|
| Column 1 | `screen_left_1` | — |
| Column 2 | `screen_left_2` | — |
| Column 3 | `screen_left_3` | — |
| Column 4 | `screen_left_4` | — |
| Column 5 | `screen_right_1` | — |
| Column 6 | `screen_right_2` | — |
| Column 7 | `screen_right_3` | — |
| Column 8 | `screen_right_4` | — |

### Below the Screen

| Position | Encoder | Touch | Button (push) |
|----------|---------|-------|---------------|
| Left | `screen_encoder_1` | `screen_encoder_touch_1` | — |
| Center-left | `screen_encoder_2` | `screen_encoder_touch_2` | — |
| Center-right | `screen_encoder_3` | `screen_encoder_touch_3` | — |
| Right | `screen_encoder_4` | `screen_encoder_touch_4` | — |

These are the controls most relevant for screen-interactive UIs, as users expect the on-screen display to reflect what these controls are doing.

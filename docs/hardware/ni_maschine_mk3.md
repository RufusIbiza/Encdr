# NI Maschine Mk3 Hardware Reference

## Overview

| Property     | Value                                              |
| ------------ | -------------------------------------------------- |
| Manufacturer | Native Instruments                                 |
| Product      | Maschine Mk3                                       |
| VID:PID      | `0x17cc:0x1600`                                    |
| USB Speed    | High Speed (480 Mbps)                              |
| Interfaces   | 2 (control on interface 4 + screen on interface 5) |
| Screens      | 2x 480 x 272, BGR565 big-endian (left and right)   |

The Maschine Mk3 is a pad-based production controller with two 4.3" color displays, eight touch-sensitive rotary encoders, a large main encoder with directional buttons, 16 velocity-sensitive RGB pads in a 4x4 grid, group selectors A-H, a capacitive touchstrip, transport controls, and extensive mode/navigation buttons.

---

## Getting Started

Because the Mk3 exposes its controls and screens on two completely separate USB interfaces, it requires some special setup compared to other controllers.

### Linux Requirements

The Linux HID subsystem will automatically claim the Mk3's control interface. Before `encdr` can communicate with the device, this kernel driver must be detached.

1. **Udev Rules (Required):** You must have a `udev` rule to allow your user account to access the USB device, otherwise `encdr` will fail with permission errors.
   Create a file `/etc/udev/rules.d/99-ni-controllers.rules`:
   ```bash
   SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"
   ```
   Then reload the rules:
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```
2. **Automatic Detachment:** `encdr` uses a `detach_kernel_driver` quirk specifically for this device. As long as the `udev` rule is in place, `encdr` will automatically detach the kernel driver and claim the interface for you.

### Running the Example

To verify everything is working, run the Mk3 screen test example. This example initializes the device, captures input from the encoders and buttons, and renders an interactive UI across both color displays.

```bash
cargo run -p encdr-examples --bin mk3_screen_test
```

---

## Physical Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                          │
│  [Channel] [Plugin]  [Top1][Top2][Top3][Top4][Top5][Top6][Top7][Top8]   │
│  [Arranger][Mixer ]                                                      │
│  [Browser] [Sampling]                                                    │
│                                                                          │
│  ┌───────────────────────┐  ┌───────────────────────┐                   │
│  │                       │  │                       │                    │
│  │     LEFT SCREEN       │  │     RIGHT SCREEN      │    [◄] [▲] [►]   │
│  │      480 x 272        │  │      480 x 272        │        (Main)     │
│  │                       │  │                       │        [▼]        │
│  └───────────────────────┘  └───────────────────────┘                   │
│  (enc1) (enc2) (enc3) (enc4) (enc5) (enc6) (enc7) (enc8)              │
│                                                                          │
│  [◄Left ] [◄Right]  [Notes]  [Volume] [Swing] [Tempo]                  │
│  [File  ] [Settings]                                                     │
│  [Auto  ] [Macro   ]                                                     │
│                                                                          │
│  [NoteRpt] [Lock]  [PadMode] [Keyboard] [Chords] [Step]                │
│  [Pitch]   [Mod]   [FixdVel] [Scene]    [Pattern][Events]              │
│  [Perform]         [Variation][Duplicate][Select] [Solo] [Mute]         │
│                                                                          │
│  ═══════════════════ TOUCHSTRIP ═══════════════════                      │
│                                                                          │
│  [Grp A] [Grp B] [Grp C] [Grp D]                                       │
│  [Grp E] [Grp F] [Grp G] [Grp H]                                       │
│                                                                          │
│  [Pad13] [Pad14] [Pad15] [Pad16]   [Restart] [Erase] [Tap] [Follow]   │
│  [Pad 9] [Pad10] [Pad11] [Pad12]   [Play]    [Rec]   [Stop]           │
│  [Pad 5] [Pad 6] [Pad 7] [Pad 8]                                       │
│  [Pad 1] [Pad 2] [Pad 3] [Pad 4]   [Shift]                            │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## USB Interfaces

| Interface | ID        | Number | Endpoint In        | Endpoint Out       | Type                                |
| --------- | --------- | ------ | ------------------ | ------------------ | ----------------------------------- |
| Control   | `control` | 4      | `0x83` (interrupt) | `0x03` (interrupt) | Buttons, encoders, touchstrip, LEDs |
| Screen    | `screen`  | 5      | —                  | `0x04` (bulk)      | Screen pixel data                   |

**Quirk: `dual_handle`** — The Mk3 requires separate USB interface handles (one per interface). Both are claimed from the same USB device.

**Quirk: `detach_kernel_driver`** — On Linux, the HID kernel driver must be detached before claiming the interface.

---

## Input Controls

The Mk3 sends two packet types on the control interface, discriminated by size:

### Buttons Packet (32 bytes)

Sent on the control interface interrupt endpoint when any button, touch sensor, encoder, or touchstrip state changes.

#### Buttons (63 total)

| Name                 | Byte | Mask   | Description                             |
| -------------------- | ---- | ------ | --------------------------------------- |
| `channel`            | 1    | `0x01` | Channel button (left column)            |
| `plugin`             | 1    | `0x02` | Plugin button (right column)            |
| `arranger`           | 1    | `0x04` | Arranger button (left column)           |
| `mixer`              | 1    | `0x08` | Mixer button (right column)             |
| `browser`            | 1    | `0x10` | Browser button (left column)            |
| `sampling`           | 1    | `0x20` | Sampling button (right column)          |
| `arrow_left`         | 1    | `0x40` | Left arrow button (left column)         |
| `arrow_right`        | 1    | `0x80` | Right arrow button (right column)       |
| `file`               | 2    | `0x01` | File button (left column)               |
| `settings`           | 2    | `0x02` | Settings button (right column)          |
| `auto`               | 2    | `0x04` | Auto button (left column)               |
| `macro`              | 2    | `0x08` | Macro button (right column)             |
| `top_0`              | 2    | `0x10` | Top button 1 (leftmost, above screens)  |
| `top_1`              | 2    | `0x20` | Top button 2                            |
| `top_2`              | 2    | `0x40` | Top button 3                            |
| `top_3`              | 2    | `0x80` | Top button 4                            |
| `top_4`              | 3    | `0x01` | Top button 5                            |
| `top_5`              | 3    | `0x02` | Top button 6                            |
| `top_6`              | 3    | `0x04` | Top button 7                            |
| `top_7`              | 3    | `0x08` | Top button 8 (rightmost, above screens) |
| `encoder_main_press` | 3    | `0x10` | Main encoder push                       |
| `encoder_main_up`    | 3    | `0x20` | Main encoder D-pad up                   |
| `encoder_main_down`  | 3    | `0x40` | Main encoder D-pad down                 |
| `encoder_main_left`  | 3    | `0x80` | Main encoder D-pad left                 |
| `encoder_main_right` | 4    | `0x01` | Main encoder D-pad right                |
| `notes`              | 4    | `0x02` | Notes button                            |
| `volume`             | 4    | `0x04` | Volume button                           |
| `swing`              | 4    | `0x08` | Swing button                            |
| `tempo`              | 4    | `0x10` | Tempo button                            |
| `note_repeat`        | 4    | `0x20` | Note Repeat button                      |
| `lock`               | 4    | `0x40` | Lock button                             |
| `pitch`              | 4    | `0x80` | Pitch button                            |
| `mod`                | 5    | `0x01` | Mod button                              |
| `perform`            | 5    | `0x02` | Perform button                          |
| `pad_mode`           | 5    | `0x04` | Pad Mode button                         |
| `keyboard`           | 5    | `0x08` | Keyboard button                         |
| `chords`             | 5    | `0x10` | Chords button                           |
| `step`               | 5    | `0x20` | Step button                             |
| `fixed_vel`          | 5    | `0x40` | Fixed Velocity button                   |
| `scene`              | 5    | `0x80` | Scene button                            |
| `pattern`            | 6    | `0x01` | Pattern button                          |
| `events`             | 6    | `0x02` | Events button                           |
| `variations`         | 6    | `0x04` | Variations button                       |
| `duplicate`          | 6    | `0x08` | Duplicate button                        |
| `select`             | 6    | `0x10` | Select button                           |
| `solo`               | 6    | `0x20` | Solo button                             |
| `mute`               | 6    | `0x40` | Mute button                             |
| `group_a`            | 6    | `0x80` | Group A selector                        |
| `group_b`            | 7    | `0x01` | Group B selector                        |
| `group_c`            | 7    | `0x02` | Group C selector                        |
| `group_d`            | 7    | `0x04` | Group D selector                        |
| `group_e`            | 7    | `0x08` | Group E selector                        |
| `group_f`            | 7    | `0x10` | Group F selector                        |
| `group_g`            | 7    | `0x20` | Group G selector                        |
| `group_h`            | 7    | `0x40` | Group H selector                        |
| `restart`            | 7    | `0x80` | Restart transport                       |
| `erase`              | 8    | `0x01` | Erase button                            |
| `tap`                | 8    | `0x02` | Tap tempo button                        |
| `follow`             | 8    | `0x04` | Follow button                           |
| `play`               | 8    | `0x08` | Play transport                          |
| `rec`                | 8    | `0x10` | Record transport                        |
| `stop`               | 8    | `0x20` | Stop transport                          |
| `shift`              | 8    | `0x40` | Shift modifier                          |

#### Touch Sensors (9 total)

Touch sensors emit `Event::Touch` with `touched: bool`.

| Name                 | Byte | Mask   | Description            |
| -------------------- | ---- | ------ | ---------------------- |
| `encoder_main_touch` | 9    | `0x01` | Main encoder touch     |
| `encoder_touch_0`    | 9    | `0x02` | Screen encoder 1 touch |
| `encoder_touch_1`    | 9    | `0x04` | Screen encoder 2 touch |
| `encoder_touch_2`    | 9    | `0x08` | Screen encoder 3 touch |
| `encoder_touch_3`    | 9    | `0x10` | Screen encoder 4 touch |
| `encoder_touch_4`    | 9    | `0x20` | Screen encoder 5 touch |
| `encoder_touch_5`    | 9    | `0x40` | Screen encoder 6 touch |
| `encoder_touch_6`    | 9    | `0x80` | Screen encoder 7 touch |
| `encoder_touch_7`    | 10   | `0x01` | Screen encoder 8 touch |

#### Encoders (9 total)

##### Main Encoder (1)

Notched push-encoder with 4-bit wraparound counter. Emits `Event::Encoder` with `delta: i32` (typically +1 or -1).

| Name           | Byte | Bits | Bit Offset | Encoding |
| -------------- | ---- | ---- | ---------- | -------- |
| `encoder_main` | 11   | 4    | 0          | `wrap16` |

##### Screen Encoders (8)

Smooth, touch-sensitive encoders below the screens. Emit `Event::EncoderFine` with `delta: f32` (high resolution, scaled by 1000.0).

| Name               | Bytes    | Encoding   | Scale  |
| ------------------ | -------- | ---------- | ------ |
| `screen_encoder_0` | [12, 13] | `signed16` | 1000.0 |
| `screen_encoder_1` | [14, 15] | `signed16` | 1000.0 |
| `screen_encoder_2` | [16, 17] | `signed16` | 1000.0 |
| `screen_encoder_3` | [18, 19] | `signed16` | 1000.0 |
| `screen_encoder_4` | [20, 21] | `signed16` | 1000.0 |
| `screen_encoder_5` | [22, 23] | `signed16` | 1000.0 |
| `screen_encoder_6` | [24, 25] | `signed16` | 1000.0 |
| `screen_encoder_7` | [26, 27] | `signed16` | 1000.0 |

#### Touchstrip (1 slider + 1 touch)

The touchstrip is a horizontal capacitive strip. It emits both a position slider and a touch event. Touch is detected as the 16-bit position value being non-zero.

| Name               | Type            | Bytes    | Description                             |
| ------------------ | --------------- | -------- | --------------------------------------- |
| `touchstrip_touch` | `Event::Touch`  | [30, 31] | Multi-byte touch (value > 0)            |
| `touchstrip`       | `Event::Slider` | [30, 31] | Position, normalized 0.0-1.0 (max 1024) |

Emits `Event::Slider` with `value: f32` normalized 0.0-1.0.

### Pads Packet (128 bytes, double-pumped)

The 16 velocity-sensitive pads send pressure data in a 128-byte packet on the control interface. The packet is "double-pumped" — it contains two snapshots of all 16 pad pressures per USB frame, giving twice the temporal resolution. Each pad pressure value is a 16-bit unsigned integer (little-endian). A value of 0 indicates no pressure; higher values indicate harder presses. The packet layout is:

```
[2-byte pad 1, snapshot A] [2-byte pad 2, snapshot A] ... [2-byte pad 16, snapshot A]
[2-byte pad 1, snapshot B] [2-byte pad 2, snapshot B] ... [2-byte pad 16, snapshot B]
... (repeated to fill 128 bytes)
```

Each snapshot contains 16 pads x 2 bytes = 32 bytes. The double-pumped format provides 4 snapshots in the 128-byte packet.

Pads emit `Event::Pressure` with `value: f32` normalized 0.0-1.0.

| Name     | Pad Index | Grid Position               |
| -------- | --------- | --------------------------- |
| `pad_0`  | 0         | Row 4, Col 1 (bottom-left)  |
| `pad_1`  | 1         | Row 4, Col 2                |
| `pad_2`  | 2         | Row 4, Col 3                |
| `pad_3`  | 3         | Row 4, Col 4 (bottom-right) |
| `pad_4`  | 4         | Row 3, Col 1                |
| `pad_5`  | 5         | Row 3, Col 2                |
| `pad_6`  | 6         | Row 3, Col 3                |
| `pad_7`  | 7         | Row 3, Col 4                |
| `pad_8`  | 8         | Row 2, Col 1                |
| `pad_9`  | 9         | Row 2, Col 2                |
| `pad_10` | 10        | Row 2, Col 3                |
| `pad_11` | 11        | Row 2, Col 4                |
| `pad_12` | 12        | Row 1, Col 1 (top-left)     |
| `pad_13` | 13        | Row 1, Col 2                |
| `pad_14` | 14        | Row 1, Col 3                |
| `pad_15` | 15        | Row 1, Col 4 (top-right)    |

**Pedal input:** The Mk3 has a 1/4" pedal jack for sustain. Pedal state is reported in the buttons packet.

---

## LED Output

The Mk3 uses two separate LED output buffers, each sent on the control interface interrupt out endpoint (`0x03`).

### Button LEDs (62 bytes, prefix `0x80`)

Single-color LEDs set with `LedValue::Single(brightness)` where brightness is 0-255.

| Name                 | Offset | Description                          |
| -------------------- | ------ | ------------------------------------ |
| `top_0`              | 0      | Top button 1 (leftmost)              |
| `top_1`              | 1      | Top button 2                         |
| `top_2`              | 2      | Top button 3                         |
| `top_3`              | 3      | Top button 4                         |
| `top_4`              | 4      | Top button 5                         |
| `top_5`              | 5      | Top button 6                         |
| `top_6`              | 6      | Top button 7                         |
| `top_7`              | 7      | Top button 8 (rightmost)             |
| `channel`            | 8      | Channel button                       |
| `plugin`             | 9      | Plugin button                        |
| `arranger`           | 10     | Arranger button                      |
| `mixer`              | 11     | Mixer button                         |
| `browser`            | 12     | Browser button                       |
| `sampling`           | 13     | Sampling button (HSV color)          |
| `arrow_left`         | 14     | Left arrow button                    |
| `arrow_right`        | 15     | Right arrow button                   |
| `file`               | 16     | File button                          |
| `settings`           | 17     | Settings button                      |
| `auto`               | 18     | Auto button                          |
| `macro`              | 19     | Macro button                         |
| `encoder_main_up`    | 20     | Main encoder D-pad up (HSV color)    |
| `encoder_main_left`  | 21     | Main encoder D-pad left (HSV color)  |
| `encoder_main_right` | 22     | Main encoder D-pad right (HSV color) |
| `encoder_main_down`  | 23     | Main encoder D-pad down (HSV color)  |
| `notes`              | 24     | Notes button                         |
| `volume`             | 25     | Volume button                        |
| `swing`              | 26     | Swing button                         |
| `tempo`              | 27     | Tempo button                         |
| `note_repeat`        | 28     | Note Repeat button                   |
| `lock`               | 29     | Lock button                          |
| `pitch`              | 30     | Pitch button                         |
| `mod`                | 31     | Mod button                           |
| `perform`            | 32     | Perform button                       |
| `pad_mode`           | 33     | Pad Mode button                      |
| `keyboard`           | 34     | Keyboard button                      |
| `chords`             | 35     | Chords button                        |
| `step`               | 36     | Step button                          |
| `fixed_vel`          | 37     | Fixed Velocity button                |
| `scene`              | 38     | Scene button                         |
| `pattern`            | 39     | Pattern button                       |
| `events`             | 40     | Events button                        |
| `variations`         | 41     | Variations button                    |
| `duplicate`          | 42     | Duplicate button                     |
| `select`             | 43     | Select button                        |
| `solo`               | 44     | Solo button                          |
| `mute`               | 45     | Mute button                          |
| `restart`            | 46     | Restart button                       |
| `erase`              | 47     | Erase button                         |
| `tap`                | 48     | Tap button                           |
| `follow`             | 49     | Follow button                        |
| `play`               | 50     | Play button                          |
| `rec`                | 51     | Rec button                           |
| `stop`               | 52     | Stop button                          |
| `shift`              | 53     | Shift button                         |

**Note:** Some button LEDs (groups A-H, encoder directional buttons, sampling) use HSV color encoding rather than simple brightness. For these, see the Pad/Group LED buffer below.

### Pad & Group LEDs (prefix `0x81`)

A separate LED buffer with prefix `0x81` controls the touchstrip LEDs, group selector LEDs, and pad RGB LEDs. This buffer uses HSV color encoding for color LEDs.

#### Touchstrip LEDs (25)

25 individually addressable LEDs along the touchstrip.

| Name         | Offset | Count | Description                          |
| ------------ | ------ | ----- | ------------------------------------ |
| `touchstrip` | 0      | 25    | Touchstrip LED strip (left to right) |

Set with `set_led_strip(device_id, "touchstrip", &[u8; 25])`.

#### Group Selector LEDs (8, HSV color)

Group LEDs use HSV color encoding with 3 bytes per LED: Hue, Saturation, Value (brightness).

| Name      | H Offset | S Offset | V Offset | Description      |
| --------- | -------- | -------- | -------- | ---------------- |
| `group_a` | 25       | 26       | 27       | Group A selector |
| `group_b` | 28       | 29       | 30       | Group B selector |
| `group_c` | 31       | 32       | 33       | Group C selector |
| `group_d` | 34       | 35       | 36       | Group D selector |
| `group_e` | 37       | 38       | 39       | Group E selector |
| `group_f` | 40       | 41       | 42       | Group F selector |
| `group_g` | 43       | 44       | 45       | Group G selector |
| `group_h` | 46       | 47       | 48       | Group H selector |

Set with `LedValue::Hsv { h, s, v }`.

#### Pad LEDs (16, HSV color)

Each pad has an HSV LED with independent hue, saturation, and value (brightness) channels.

| Name     | H Offset | S Offset | V Offset | Grid Position               |
| -------- | -------- | -------- | -------- | --------------------------- |
| `pad_0`  | 49       | 50       | 51       | Row 4, Col 1 (bottom-left)  |
| `pad_1`  | 52       | 53       | 54       | Row 4, Col 2                |
| `pad_2`  | 55       | 56       | 57       | Row 4, Col 3                |
| `pad_3`  | 58       | 59       | 60       | Row 4, Col 4 (bottom-right) |
| `pad_4`  | 61       | 62       | 63       | Row 3, Col 1                |
| `pad_5`  | 64       | 65       | 66       | Row 3, Col 2                |
| `pad_6`  | 67       | 68       | 69       | Row 3, Col 3                |
| `pad_7`  | 70       | 71       | 72       | Row 3, Col 4                |
| `pad_8`  | 73       | 74       | 75       | Row 2, Col 1                |
| `pad_9`  | 76       | 77       | 78       | Row 2, Col 2                |
| `pad_10` | 79       | 80       | 81       | Row 2, Col 3                |
| `pad_11` | 82       | 83       | 84       | Row 2, Col 4                |
| `pad_12` | 85       | 86       | 87       | Row 1, Col 1 (top-left)     |
| `pad_13` | 88       | 89       | 90       | Row 1, Col 2                |
| `pad_14` | 91       | 92       | 93       | Row 1, Col 3                |
| `pad_15` | 94       | 95       | 96       | Row 1, Col 4 (top-right)    |

Set with `LedValue::Hsv { h, s, v }`.

---

## Screens

| Property               | Value                                    |
| ---------------------- | ---------------------------------------- |
| Count                  | 2 (left and right)                       |
| Resolution             | 480 x 272 pixels each                    |
| Pixel Format           | BGR565 big-endian                        |
| Bytes per pixel        | 2                                        |
| Full frame size        | 261,120 bytes (480 * 272 * 2) per screen |
| Interface              | `screen` (interface 5)                   |
| Endpoint               | `0x04` (bulk out)                        |
| Partial update support | Yes                                      |
| Partial X alignment    | 4 pixels                                 |
| Partial Y alignment    | 2 pixels                                 |

### Screen Addressing

Both screens share the same endpoint (`0x04` bulk out on interface 5). The target screen is selected by header byte[2]:

| Screen | Byte[2] Value | Description        |
| ------ | ------------- | ------------------ |
| Left   | `0x00`        | Left 4.3" display  |
| Right  | `0x01`        | Right 4.3" display |

### Full Blit Protocol

A full screen update is sent as:

```
[20-byte header] [261,120 bytes pixel data] [8-byte footer]
```

The header is the same format as the D2, with the addition of the screen index in byte[2]:

Header (left): `84 00 00 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`
Header (right): `84 00 01 60 00 00 00 00 00 00 00 00 01 E0 01 10 00 00 FF 00`
Footer: `03 00 00 00 40 00 00 00`

### Partial Blit Protocol

Partial updates send only a rectangular region, identical to the D2 protocol but with the screen index in byte[2]:

```
[20-byte header with screen index and coordinates] [region pixel data] [8-byte footer]
```

The header template uses coordinate substitution:
- `{x_hi}`, `{x_lo}` -- X offset (big-endian u16)
- `{y_hi}`, `{y_lo}` -- Y offset
- `{w_hi}`, `{w_lo}` -- Width
- `{h_hi}`, `{h_lo}` -- Height
- `{px_half_hi}`, `{px_half_lo}` -- Half the pixel count (region_w * region_h / 2)

Coordinates must be aligned to 4 pixels horizontally and 2 pixels vertically.

---

## Controls Grouped by Screen Proximity

For building screen UIs, here are the controls physically adjacent to the screens:

### Above the Screens

| Position      | Button  | Screen |
| ------------- | ------- | ------ |
| 1 (far left)  | `top_0` | Left   |
| 2             | `top_1` | Left   |
| 3             | `top_2` | Left   |
| 4             | `top_3` | Left   |
| 5             | `top_4` | Right  |
| 6             | `top_5` | Right  |
| 7             | `top_6` | Right  |
| 8 (far right) | `top_7` | Right  |

### Below the Screens

| Position      | Encoder            | Touch             | Screen |
| ------------- | ------------------ | ----------------- | ------ |
| 1 (far left)  | `screen_encoder_0` | `encoder_touch_0` | Left   |
| 2             | `screen_encoder_1` | `encoder_touch_1` | Left   |
| 3             | `screen_encoder_2` | `encoder_touch_2` | Left   |
| 4             | `screen_encoder_3` | `encoder_touch_3` | Left   |
| 5             | `screen_encoder_4` | `encoder_touch_4` | Right  |
| 6             | `screen_encoder_5` | `encoder_touch_5` | Right  |
| 7             | `screen_encoder_6` | `encoder_touch_6` | Right  |
| 8 (far right) | `screen_encoder_7` | `encoder_touch_7` | Right  |

These are the controls most relevant for screen-interactive UIs, as users expect the on-screen display to reflect what these controls are doing. The first four encoders and top buttons correspond to the left screen; the last four correspond to the right screen.

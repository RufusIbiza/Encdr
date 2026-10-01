# NI Maschine Jam Hardware Reference

## Overview

| Property     | Value                                                              |
| ------------ | ------------------------------------------------------------------ |
| Manufacturer | Native Instruments                                                 |
| Product      | Maschine Jam                                                       |
| VID:PID      | `0x17cc:0x1500`                                                    |
| USB Speed    | Full Speed (12 Mbps) / High Speed                                  |
| Interfaces   | 1 (HID Control interface 0)                                        |
| Screens      | None (LED-driven control surface)                                  |

The Maschine Jam is a production and performance controller featuring an 8x8 multicolour Click-Pad matrix (64 RGB click-pads), 8 dual-touch Smart Strips (touchstrips with 11-segment LED bar meters), an endless notched encoder with capacitive touch detection, 8 RGB Group buttons (A–H), 8 numbered top buttons (1–8), transport controls, D-Pad navigation, stereo LED level meters, and dedicated performance mode selectors.

---

## Getting Started

### Linux Requirements

Under Linux, the kernel automatically binds generic `hidraw` to the Maschine Jam. `encdr` automatically applies the `detach_kernel_driver` quirk to claim Interface 0.

1. **Udev Rules:**
   Add to `/etc/udev/rules.d/99-ni-controllers.rules`:
   ```bash
   SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"
   SUBSYSTEM=="hidraw", ATTRS{idVendor}=="17cc", ATTRS{idProduct}=="1500", MODE="0666"
   ```
2. **Reload Rules:**
   ```bash
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

---

## Physical Layout

```
┌────────────────────────────────────────────────────────────────────────────┐
│ [Song]      [ 1 ] [ 2 ] [ 3 ] [ 4 ] [ 5 ] [ 6 ] [ 7 ] [ 8 ]                │
│                                                                            │
│ [Step]      ┌───┬───┬───┬───┬───┬───┬───┬───┐          [Master] [Group]    │
│ [Pad Mode]  │   │   │   │   │   │   │   │   │          [In   ] [Phones]    │
│ [Clear]     ├───┼───┼───┼───┼───┼───┼───┼───┤                              │
│ [Duplicate] │   │   │   │   │   │   │   │   │            ( Encoder )       │
│             ├───┼───┼───┼───┼───┼───┼───┼───┤                              │
│    [▲]      │   │   │   │   │   │   │   │   │          [Browse] [Macro]    │
│ [◄]   [►]   ├───┼───┼───┼───┼───┼───┼───┼───┤          [Level ] [Aux  ]    │
│    [▼]      │      8x8 CLICK-PAD MATRIX     │          [Control][Auto ]    │
│             ├───┼───┼───┼───┼───┼───┼───┼───┤          [Perform][Notes]    │
│ [Note Repeat│   │   │   │   │   │   │   │   │          [Lock  ] [Tune ]    │
│             ├───┼───┼───┼───┼───┼───┼───┼───┤          [Swing ]            │
│             │   │   │   │   │   │   │   │   │                              │
│             ├───┼───┼───┼───┼───┼───┼───┼───┤                              │
│             │   │   │   │   │   │   │   │   │                              │
│             └───┴───┴───┴───┴───┴───┴───┴───┘                              │
│                                                                            │
│             [ A ] [ B ] [ C ] [ D ] [ E ] [ F ] [ G ] [ H ]                │
│                                                                            │
│             │ | │ │ | │ │ | │ │ | │ │ | │ │ | │ │ | │ │ | │                │
│             │ | │ │ | │ │ | │ │ | │ │ | │ │ | │ │ | │ │ | │                │
│             │ | │ │ | │ │ | │ │ | │ │ | │ │ | │ │ | │ │ | │                │
│             └───┘ └───┘ └───┘ └───┘ └───┘ └───┘ └───┘ └───┘                │
│                   8 DUAL-TOUCH SMART STRIPS (TOUCHSTRIPS)                  │
│                                                                            │
│ [Shift] [Play] [Rec] [ ◄ ] [ ► ] [Tempo] [Grid] [Solo] [Mute] [Select]     │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## USB Interfaces & Endpoints

| Interface | ID        | Number | Endpoint In        | Endpoint Out       | Type               | Description                        |
| --------- | --------- | ------ | ------------------ | ------------------ | ------------------ | ---------------------------------- |
| Control   | `control` | 0      | `0x81` (interrupt) | `0x01` (interrupt) | HID Control        | Buttons, encoder, strips, LEDs     |

---

## Input Reports (Interface 0, Endpoint `0x81`)

Maschine Jam produces two distinct input packets on interrupt endpoint `0x81`, differentiated by their payload size:

### 1. Buttons & Grid Packet (Size 17 bytes)

Contains the notched rotary encoder, capacitive touch, all 39 surrounding buttons, and the 64-pad 8x8 matrix.

#### A. Rotary Encoder
* **Encoder notched movement:** `data[1] & 0x0F` (4-bit counter, `wrap16`).
* **Encoder capacitive touch:** Byte 16, mask `0x08` (`encoder_touch`).
* **Encoder push-switch:** Byte 16, mask `0x10` (`encoder_press`).

#### B. Function & Navigation Buttons
| Name | Byte | Mask | Description |
| ---- | ---- | ---- | ----------- |
| `song` | 2 | `0x01` | Song mode |
| `top_1` .. `top_7` | 2 | `0x02`, `0x04`, `0x08`, `0x10`, `0x20`, `0x40`, `0x80` | Numbered Top buttons 1–7 |
| `top_8` | 3 | `0x01` | Numbered Top button 8 |
| `step` | 3 | `0x02` | Step sequencer mode |
| `pad_mode` | 3 | `0x04` | Pad mode selector |
| `clear` | 3 | `0x08` | Pattern / event clear |
| `duplicate` | 3 | `0x10` | Pattern / clip duplicate |
| `dpad_up` | 3 | `0x20` | D-Pad navigation Up |
| `dpad_left` | 3 | `0x40` | D-Pad navigation Left |
| `dpad_right` | 3 | `0x80` | D-Pad navigation Right |
| `dpad_down` | 4 | `0x01` | D-Pad navigation Down |
| `note_repeat` | 4 | `0x02` | Note Repeat / Arp |
| `group_a` .. `group_f` | 12 | `0x04`, `0x08`, `0x10`, `0x20`, `0x40`, `0x80` | Group selectors A through F |
| `group_g`, `group_h` | 13 | `0x01`, `0x02` | Group selectors G and H |
| `master` | 13 | `0x04` | Master channel select |
| `group` | 13 | `0x08` | Group channel select |
| `in` | 13 | `0x10` | Input monitoring |
| `headphones` | 13 | `0x20` | Cue / headphone monitor |
| `browse` | 13 | `0x40` | Sound / preset browser |
| `macro` | 13 | `0x80` | Macro parameter control |
| `level` | 14 | `0x01` | Volume level mode |
| `aux` | 14 | `0x02` | Aux send mode |
| `control` | 14 | `0x04` | Control parameter mode |
| `auto` | 14 | `0x08` | Automation record |
| `perform` | 14 | `0x10` | Perform FX mode |
| `notes` | 14 | `0x20` | Notes / scales mode |
| `lock` | 14 | `0x40` | Parameter snapshot lock |
| `tune` | 14 | `0x80` | Pitch / tune mode |
| `swing` | 15 | `0x01` | Groove swing amount |
| `shift` | 15 | `0x02` | Secondary shift function |
| `play` | 15 | `0x04` | Transport Play |
| `rec` | 15 | `0x08` | Transport Record |
| `left` | 15 | `0x10` | Page / bank Left |
| `right` | 15 | `0x20` | Page / bank Right |
| `tempo` | 15 | `0x40` | Project tempo / tap |
| `grid` | 15 | `0x80` | Grid resolution selector |
| `solo` | 16 | `0x01` | Track Solo |
| `mute` | 16 | `0x02` | Track Mute |
| `select` | 16 | `0x04` | Sound / event select |
| `footswitch` | 16 | `0x20` | Rear footswitch jack |

#### C. 8x8 Click-Pad Matrix
The 64 matrix buttons are packed in bytes 4 through 12. For each row $r \in [1..8]$:
* Columns 1–6: Bits 2..7 (`0x04, 0x08, 0x10, 0x20, 0x40, 0x80`) in `data[3 + r]`
* Column 7: Bit 0 (`0x01`) in `data[4 + r]`
* Column 8: Bit 1 (`0x02`) in `data[4 + r]`

Named in Encdr as `matrix_{row}_{col}` (e.g. `matrix_1_1` through `matrix_8_8`).

---

### 2. Touchstrips Packet (Size 49 bytes)

Emitted continuously when fingers touch or move along any of the 8 Smart Strips:
* `data[0]`: Report ID `0x02`
* 8 touchstrips, 6 bytes each (offset $1 + i \times 6$ for strip $i \in [0..7]$):
  * `[offset + 0, offset + 1]`: 16-bit activity timestamp
  * `[offset + 2, offset + 3]`: Primary finger position (`0..1023`, 10-bit LE). Value is `0` when released.
  * `[offset + 4, offset + 5]`: Secondary finger position (`0..1023`, 10-bit LE) for dual-touch gestures.

Items exposed in Encdr:
* `touchstrip_1` .. `touchstrip_8`: Primary normalized slider value (`0.0` .. `1.0`).
* `touchstrip_1_dual` .. `touchstrip_8_dual`: Secondary touch position (`0.0` .. `1.0`).
* `touchstrip_1_touch` .. `touchstrip_8_touch`: Touch presence event (`true` on contact, `false` on release).

---

## Output Reports & LED Control (Interface 0, Endpoint `0x01`)

Maschine Jam accepts three output reports on interrupt endpoint `0x01`:

### 1. Surrounding Buttons & VU Meters (Report `0x80`, 66 bytes)

Monochrome and single-color LEDs for transport, modes, and level meters:
* `song` (offset 1), `step` (offset 2), `pad_mode` (offset 3), `clear` (offset 4), `duplicate` (offset 5)
* `dpad_up` (offset 6), `dpad_left` (offset 7), `dpad_right` (offset 8), `dpad_down` (offset 9)
* `note_repeat` (offset 10), `master` (offset 11), `group` (offset 12), `in` (offset 13), `headphones` (offset 15)
* `browse` (offset 17), `macro` (offset 18), `level` (offset 19), `aux` (offset 20), `control` (offset 21), `auto` (offset 22)
* `perform` (offset 23), `notes` (offset 24), `lock` (offset 25), `tune` (offset 26), `swing` (offset 27), `shift` (offset 28)
* `play` (offset 29), `rec` (offset 30), `left` (offset 31), `right` (offset 32), `tempo` (offset 33), `grid` (offset 34)
* `solo` (offset 35), `mute` (offset 36), `select` (offset 37)
* **Stereo VU Level Meters:**
  * `vu_meter_l`: 8 LEDs (offsets 38–45)
  * `vu_meter_r`: 8 LEDs (offsets 46–53)

Values: `LedValue::Single(0..127)` or `LedValue::Off`.

### 2. Click-Pad Matrix & RGB Groups (Report `0x81`, 81 bytes)

Full RGB palette control for top buttons, 8x8 matrix, and group buttons:
* **Top Buttons 1–8:** Offsets 1–8 (`top_1` .. `top_8`).
* **8x8 Matrix (64 Pads):** Offsets 9–72 (`matrix_1_1` .. `matrix_8_8`).
* **Group Buttons A–H:** Offsets 73–80 (`group_a` .. `group_h`).

Values: Uses Native Instruments' packed palette byte format (`LedValue::Rgb` automatically converts via `to_ni_palette_byte`).

### 3. Smart Strip LED Meters (Report `0x82`, 90 bytes)

Controls the 11-segment LED bar graphs above each of the 8 Smart Strips (88 LEDs total):
* `touchstrip_meter_1`: offset 1, count 11
* `touchstrip_meter_2`: offset 12, count 11
* `touchstrip_meter_3`: offset 23, count 11
* `touchstrip_meter_4`: offset 34, count 11
* `touchstrip_meter_5`: offset 45, count 11
* `touchstrip_meter_6`: offset 56, count 11
* `touchstrip_meter_7`: offset 67, count 11
* `touchstrip_meter_8`: offset 78, count 11

---

## Example: Pad Scroller & Sine Meters

An interactive demonstration is available in `examples/maschine_jam/jam_scroller.rs`:
- Scrolls the word **"Encdr"** across the 8x8 Click-Pad matrix in purple on an orange background.
- Animates scrolling sine waves across the 8 Smart Strip 11-segment LED meters at the bottom at the exact same horizontal speed as the text.
- Provides real-time console logging and visual feedback for all 103 buttons, pads, encoder rotation/touch/press, and touchstrips.

```bash
cargo run -p encdr-examples --bin jam_scroller
```


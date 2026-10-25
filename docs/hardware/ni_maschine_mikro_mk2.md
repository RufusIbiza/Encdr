# NI Maschine Mikro Mk2 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Maschine Mikro Mk2                       |
| VID:PID      | `0x17cc:0x1200`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | 1 (128×64 monochrome graphic LCD)        |

The Maschine Mikro Mk2 features multi-color RGB backlighting for all 16 velocity- and pressure-sensitive pads, function button backlighting, an RGB `GROUP` LED, a 128×64 high-contrast monochrome graphic display, and an endless push-rotary encoder.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [ 128x64 MONO LCD ]       ( Encoder / Push ) |
|                                              |
| [F1] [F2] [F3] [Group] [Browse] [Sample] [Step] |
|                                              |
| ┌──────┬──────┬──────┬──────┐                |
| │Pad 13│Pad 14│Pad 15│Pad 16│                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 9 │Pad 10│Pad 11│Pad 12│                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 5 │Pad 6 │Pad 7 │Pad 8 │                |
| ├──────┼──────┼──────┼──────┤                |
| │Pad 1 │Pad 2 │Pad 3 │Pad 4 │                |
| └──────┴──────┴──────┴──────┘                |
|                                              |
| [Restart] [Play] [Rec] [Erase] [Grid] [Note] |
| [Shift]   [Nav]  [View]                      |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
  - *Note:* Because both screens and LEDs output over Interrupt OUT endpoint `0x01` on Interface 0, Encdr automatically multiplexes OUT transfers through an endpoint worker so that concurrent screen updates and LED updates never collide.
* **Input Packets:**
  - `buttons` (Report ID `0x01`, 6 bytes): 32 button bits across bytes 1–4, and rotary push encoder (low nibble, 4-bit `wrap16`) in byte 5.
  - `pads` (Report ID `0x20`, 64 bytes): Continuous 16-channel 12-bit ADC pad pressure stream (`PadStreamState` with sliding median filter and active hold tracking).
* **Output Packet:**
  - Report ID `0x80`, 78 bytes payload (`linear_7bit`, `0..127`): 30 button LED bytes (including RGB `GROUP` LED at offsets 8, 9, 10), followed by 16 RGB pad triples from offset 30 in hardware pad-index order (`pad_13` .. `pad_4`).
* **Screen Protocol:**
  - Report ID `0xE0`, 128×64 1-bit monochrome display using `ScreenProtocol::NiMikroMk2`.
  - Sent as four 265-byte page transfers (`0xE0` report ID + 8-byte LE header `[x=0, y_page, width=128, height_pages=2]` with `y_page` $\in \{0, 2, 4, 6\}$ + 256 column-page data bytes).

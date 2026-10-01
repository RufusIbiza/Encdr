# NI Traktor Kontrol Z1 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Traktor Kontrol Z1                       |
| VID:PID      | `0x17cc:0x1210`                          |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | None (LED VU meters)                     |

The Traktor Kontrol Z1 is a compact 2-channel mixing controller with high-grade line faders, a crossfader, dedicated 3-band EQs, filter/FX knobs, headphone cue buttons, and dual 7-segment LED VU level meters.

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [Main]               (Gain A)       (Gain B) |
|                                              |
|                      (High A)       (High B) |
|                      (Mid A)        (Mid B)  |
|                      (Low A)        (Low B)  |
|                                              |
|                      [Mode A]       [Mode B] |
|                      (Filter A)     (Filter B)|
|                                              |
|                      [VU L A]       [VU R B] |
|                                              |
| [Cue Mix] [Cue Vol]  [Cue A]        [Cue B]  |
|                                              |
|                      │  ▲  │        │  ▲  │  |
|                      │  █  │        │  █  │  |
|                      │  ▼  │        │  ▼  │  |
|                      Fader A        Fader B  |
|                                              |
|                  ◄─── Crossfader ───►        |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packet:** 28 bytes containing 13 analog faders and potentiometers (12-bit LE) and 4 mode/cue buttons.
* **Output Packet:** 32 bytes driving the 4 button LEDs and the dual 7-segment stereo VU meters.

# NI Traktor Kontrol X1 Mk1 Hardware Reference

## Overview

| Property     | Value                                    |
| ------------ | ---------------------------------------- |
| Manufacturer | Native Instruments                       |
| Product      | Traktor Kontrol X1 Mk1                   |
| VID:PID      | `0x17cc:0x2305` / `0x17cc:0x1000`        |
| USB Speed    | Full Speed (12 Mbps)                     |
| Interfaces   | 1 (HID Control interface 0)              |
| Screens      | None (LED-driven control surface)        |

The Traktor Kontrol X1 Mk1 is Native Instruments' original compact performance controller for Traktor, offering dual deck FX and transport control with 8 analog rotary knobs, 4 push encoders, and 30 dual-color backlit buttons (amber/green).

---

## Physical Layout

```
┌──────────────────────────────────────────────┐
| [FX1 On]  [ 1 ] [ 2 ] [ 3 ]   [FX2 On] [ 1 ] [ 2 ] [ 3 ] |
| (Knob 1) (Knob 2)(Knob 3)(Knob 4) (Knob 1)(Knob 2)(Knob 3)(Knob 4) |
|                                              |
| (Enc 1)  (Enc 2)             (Enc 1)  (Enc 2)|
|                                              |
| [ 1 ] [ 2 ] [ 3 ] [ 4 ]       [ 1 ] [ 2 ] [ 3 ] [ 4 ] |
| [ In ] [ Out ]                [ In ] [ Out ] |
|                                              |
| [Play] [Cue] [Sync] [Shift]   [Play] [Cue] [Sync] |
└──────────────────────────────────────────────┘
```

---

## USB Endpoints & Reports

* **Interface 0 (Control):** Interrupt IN `0x81`, Interrupt OUT `0x01`
* **Input Packet:** 24 bytes containing 8 analog potentiometer values (12-bit), 4 rotary encoders, and 30 tactile switch buttons.
* **Output Packet:** 48 bytes controlling the single-color and dual-color LED states.

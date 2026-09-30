# NI Komplete Kontrol S-Series Mk3 Hardware Reference (WIP)

## Overview

| Property     | Value                                                              |
| ------------ | ------------------------------------------------------------------ |
| Manufacturer | Native Instruments                                                 |
| Product      | Komplete Kontrol S49 Mk3 / S61 Mk3 / S88 Mk3                       |
| VID:PID      | `0x17cc:0x2100` (S49), `0x17cc:0x2110` (S61), `0x17cc:0x2120` (S88)|
| USB Speed    | High Speed (480 Mbps)                                              |
| Screen       | 1x High-Resolution Full-Color Glass Display                        |
| Protocol     | USB MIDI 2.0 / 1.0 (Keys/Aftertouch) + USB HID + RODA Bulk Protocol|

The Komplete Kontrol S-Series Mk3 is Native Instruments' flagship keyboard controller featuring an on-board dual-core ARM SoC (STM32MP157) running embedded Linux, polyphonic aftertouch keybeds, a continuous unibody glass surface with a full-color display, anodized aluminum rotary encoders, 4D directional encoder, pitch and modulation wheels, multi-touch expression strip, and RGB per-key Light Guide.

---

## Hardware Protocol Definition

Extracted from the Native Instruments Komplete Kontrol 3.3.3 controller registry (`Komplete Kontrol.exe` @ `0x3e9ad98`):

```text
I-1 b28 n2 I-2 b8 WC b10 I-3 b8 W3 W2 B1 I-AA W19 O-80 B2A O-81 B1E O-82 B31 O-A0 BE O-A1 BC8 W1 B1 O-A2 B2C O-A3 B90 O-A4 B80 O-AF B2 O-F3 B1 O-F4 B20 F-D0 D1 B1C F-D8 D2 W4 B10 F-D9 B20 F-F8 W2 B6
```

### Input Reports (Interrupt Endpoint)

* **Report `0x01` (`I-1`):**
  * `b28` (40 bits = 5 bytes): Button state bitmask.
  * `n2` (2 nibbles = 1 byte): 4D encoder relative notched counter (`wrap16`).
* **Report `0x02` (`I-2`):**
  * `b8 WC b10`: Pitch and modulation expression touchstrip data.
* **Report `0x03` (`I-3`):**
  * `b8 W3 W2 B1`: Pedals and expression inputs.
* **Report `0xAA` (`I-AA`):**
  * `W19` (19 words = 38 bytes): 8 touch-sensitive continuous rotary encoders.

### Output Reports (Interrupt Endpoint)

* **Report `0x80` (`O-80`):** `B2A` (42 bytes): Button and mode indicator LEDs.
* **Report `0x81` (`O-81`):** `B1E` (30 bytes): Secondary button LEDs.
* **Report `0x82` (`O-82`):** Per-key RGB Light Guide:
  * **S49 Mk3:** `B31` (49 bytes for 49 keys)
  * **S61 Mk3:** `B3D` (61 bytes for 61 keys)
  * **S88 Mk3:** `B58` (88 bytes for 88 keys)

### Screen & System Architecture (RODA)

* **Host Integration Service:** Handled via **Hardware Connection Service (HCS)** (internally `odr_agent`), communicating over an IPC channel (`libs/roda/framework/ipc`).
* **On-Device Daemon:** Communicates with the keyboard's embedded Linux daemon (`ni-roda`).

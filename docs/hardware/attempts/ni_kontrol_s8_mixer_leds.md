# NI Kontrol S8 — Mixer LED Discovery Attempts

## Hardware layout

The S8 mixer section has per-channel buttons with LEDs: CUE (A/B/C/D), Filter On (A/B/C/D),
FX Assign 1/2 (A/B/C/D), Deck Assign (A/B/C/D), plus global Snap, Quantize, Mic Assign 1/2.

## LED packet structure (confirmed from decompiled Traktor source)

The S8 uses three 309-byte interrupt OUT packets on endpoint 0x03, interface 5:

| Prefix | Absolute LED range | Data bytes |
| ------ | ------------------ | ---------- |
| 0x80   | 0 – 117            | 118        |
| 0x81   | 118 – 235          | 118        |
| 0x82   | 236 – 308          | 73         |

Packet format: `[prefix_byte, data[0], data[1], ..., data[307]]` — always 309 bytes total,
zero-padded. `data[i]` = brightness byte for absolute LED index `(prefix_start + i)`.

**Handshake required first:** send a 2-byte interrupt OUT packet `[0xf3, 0x01]` before any
LED packets will be honoured by the mixer section. Without it the device ignores mixer LEDs.

## Confirmed LED assignments (from `S8PortDescriptor` body at `sub_14067fac0`)

Source: `Traktor.exe.bndb_pseudo_c.txt`, addresses `0x140683xxx`, via `sub_141765460`.

The function `sub_141765460(result, property, led_active, led_inactive)` binds a property to
two LED indices: one lit when the button is active, one when inactive.

Mixer channels are **1-indexed** in the S8 (channels 1–4, not 0–3).

### FX Assign buttons (0x81 packet)

| Property                     | Active LED (abs) | 0x81 offset | Inactive LED (abs) | 0x82 offset |
| ---------------------------- | ---------------- | ----------- | ------------------ | ----------- |
| mixer.channels.3.fx.assign.1 | 147              | 29          | 280                | 44          |
| mixer.channels.3.fx.assign.2 | 148              | 30          | 281                | 45          |
| mixer.channels.1.fx.assign.1 | 149              | 31          | 282                | 46          |
| mixer.channels.1.fx.assign.2 | 150              | 32          | 283                | 47          |
| mixer.snap                   | 151              | 33          | 284                | 48          |
| mixer.channels.2.fx.assign.1 | 172              | 54          | 286                | 50          |
| mixer.channels.2.fx.assign.2 | 173              | 55          | 287                | 51          |
| mixer.channels.4.fx.assign.1 | 174              | 56          | 288                | 52          |
| mixer.channels.4.fx.assign.2 | 175              | 57          | 289                | 53          |
| mixer.quant                  | 171              | 53          | 285                | 49          |
| mixer.mic.assign.1           | 146              | 28          | 298                | 62          |
| mixer.mic.assign.2           | 168              | 50          | 299                | 63          |

### VU meters (0x82 packet, 11 LEDs per channel)

| Channel | Absolute range | 0x82 offsets |
| ------- | -------------- | ------------ |
| 3       | 236 – 246      | 0 – 10       |
| 1       | 247 – 257      | 11 – 21      |
| 2       | 258 – 268      | 22 – 32      |
| 4       | 269 – 279      | 33 – 43      |

### CUE, Filter On, and Deck Assign buttons (traced via NHL2 slot table)

These are bound via NHI 4-char IDs (`sub_1417661d0`) in the S8PortDescriptor. The NHI IDs
are resolved by the NHL2 layer to absolute LED indices via two functions:

**`sub_140c9ba40`** (`Traktor.exe` address `0x140c9ba40`) — S8-specific slot-ID → absolute
LED index lookup table. This is the definitive mapping for all mixer button LEDs.

**`sub_140c9e870`** (`0x140c9e870`) — absolute LED index → packet metadata converter.
For LED range 0x70–0x7f (covers all CUE buttons): "SIMPLE" type, `arg1[1]=0; arg1[2]=0`.

The trace from "CueA" NHI ID → LED index:
1. NHI 4-char ID 0x43756541 ("CueA") → NHL2 opcode 0x77
2. NHL2 layer dispatches via `sub_140996bc0` (line 1852503): `if (rbp == 0x43756541)`
3. Calls `sub_14099a3a0` → `sub_14099ae10` → `sub_14099b120`
4. `sub_14099b120` sets slot IDs in LED tree, including slot 0x18 (CUE A active)
5. `sub_140c9ba40(0x18, arg4, 1)` → `arg4[0] = 0x77` (absolute LED index 119)
6. Packet: prefix 0x81, data byte offset 119 − 118 = **1**

| Property / NHI ID | Slot ID | Abs LED    | Prefix | Offset | Notes      |
| ----------------- | ------- | ---------- | ------ | ------ | ---------- |
| CueA / 0x43756541 | 0x18    | 0x77 = 119 | 0x81   | 1      | Active LED |
| CueB / 0x43756542 | 0x19    | 0x76 = 118 | 0x81   | 0      | Active LED |
| CueC / 0x43756543 | 0x1a    | 0x7f = 127 | 0x81   | 9      | Active LED |
| CueD / 0x43756544 | 0x1b    | 0x7e = 126 | 0x81   | 8      | Active LED |

Additional slots from `sub_140c9ba40` in the 0x70–0x7f range (likely filter/deck buttons):

| Slot ID | Abs LED    | Prefix | Offset |
| ------- | ---------- | ------ | ------ |
| 0x1c    | 0x79 = 121 | 0x81   | 3      |
| 0x1d    | 0x71 = 113 | 0x80   | 113    |
| 0x1e    | 0x70 = 112 | 0x80   | 112    |
| 0x1f    | 0x78 = 120 | 0x81   | 2      |
| 0x20    | 0x75 = 117 | 0x80   | 117    |
| 0x21    | 0x74 = 116 | 0x80   | 116    |
| 0x22    | 0x7d = 125 | 0x81   | 7      |
| 0x23    | 0x7c = 124 | 0x81   | 6      |

The NHI ID "FltA" (0x466c7441, filter_on channel A) and "TktA" (0x546b7441, traktormode)
likely use slots in the 0x1c–0x23 range above, but the exact slot-to-property mapping has
not yet been traced. **Hardware verification of the above indices is pending.**

| Property                     | NHI ID     | Decoded |
| ---------------------------- | ---------- | ------- |
| mixer.channels.1.filter_on   | 0x466c7441 | "FltA"  |
| mixer.channels.1.cue         | 0x43756541 | "CueA"  |
| mixer.channels.1.traktormode | 0x546b7441 | "TktA"  |
| mixer.channels.2.filter_on   | 0x466c7442 | "FltB"  |
| mixer.channels.2.cue         | 0x43756542 | "CueB"  |
| mixer.channels.3.filter_on   | 0x466c7443 | "FltC"  |
| mixer.channels.3.cue         | 0x43756543 | "CueC"  |
| mixer.channels.4.filter_on   | 0x466c7444 | "FltD"  |
| mixer.channels.4.cue         | 0x43756544 | "CueD"  |

## What NOT to do (confirmed wrong approaches)

### `mixer.channels.X.gain` / `.eq.high` / `.eq.mid` / `.eq.low` / `.filter` / `.volume`

These properties use `sub_141767460` (single encoder index) or `sub_141766bd0` (fader index),
**not** `sub_141765460`. They are knob/fader control bindings with no LED output. The S8 mixer
has no LED rings on EQ/gain knobs. Do not attempt to drive these as LEDs.

### 0xf3 packet — group toggle, not per-LED control

Hardware testing of the `[0xf3, ...]` packet shows it toggles the **entire mixer LED bank**
(CUE A/B/C/D + Filter A/B/C/D + Snap + Quant + Mic 1/2) as a group — not individual LEDs.
The behavior is **state-dependent** (each call inverts the current all-on/all-off state):

| Packet                           | Result                       |
| -------------------------------- | ---------------------------- |
| `[0xf3, 0x01]` (2 bytes)         | All mixer LEDs ON            |
| `[0xf3, 0x00]` (2 bytes)         | No change                    |
| `[0xf3, 0x7f, 0x00 × 10]` (12 b) | Toggles entire bank          |
| `[0xf3, 0x7f × 11]` (12 b)       | No change from current state |
| Bytes[2..11] set individually    | No individual LED responds   |

Interpretation: `0xf3` is a bulk-enable toggle, not an LED address space. Varying individual
bytes within the packet has no effect. This is not useful for per-LED control.

### Scripts that never sent USB

- `s8_discovery/src/bin/mixer_handshake_test.rs` — pure text UI, no nusb imports, never sent
  any USB packets. All button responses observed during these tests were unrelated hardware behavior.
- `s8_discovery/src/bin/mixer_offset_map.rs` — same: pure text UI stub, no USB communication.

### `led_test.rs test_mixer` (original offsets)

Used `0x81:219` for filter on. Offset 219 in the 308-byte data array corresponds to absolute
LED 118+219=337, which is out of range for the 309-LED device. No response expected.

### `mixer_discovery.rs` prefix 0x82 without handshake

All 308 offsets logged as "skip" — handshake was never sent so the device ignored all packets.

### `mixer_probe` prefix 0x82 with handshake

Swept all 308 offsets of prefix 0x82 with 300ms blink, handshake sent correctly beforehand.
Zero observations — no mixer LEDs responded. Prefix 0x82 does not control mixer LEDs.

### `led_test.rs test_mixer` (corrected offsets, 0x81)

Sent confirmed FX assign LED indices via 0x81 packets (offsets 29–33, 50, 53–57) after
handshake. No mixer LEDs responded. Prefix 0x81 does not appear to control mixer LEDs either.

### MIDI SysEx path (hardware tested — confirmed dead end)

Traced `sub_1405dbaa0` in Traktor binary — builds MIDI SysEx `[0xF0, 0x00, 0x21, 0x09, ...data..., 0xF7]`
(NI manufacturer ID). Interface 3 on the S8 is USB MIDI class (class=0x01, subclass=0x03,
bulk EP 0x02 OUT / EP 0x83 IN, 512-byte packets).

Hardware tested via `led_test midi_sysex` and `midi_raw` commands:
- `[0xF0, 0x00, 0x21, 0x09, 0xF7]` — no response
- `[0xF0, 0x00, 0x21, 0x09, 0x77, 0x7f, 0xF7]` — no response (LED index + brightness)
- `[0xF0, 0x00, 0x21, 0x09, 0x37, 0x77, 0x7f, 0xF7]` — no response (device ID guess)

No mixer LED changed in any test. MIDI is a dead end for mixer LED control.
The MIDI interface is consistent with MIDI remapping mode only, not LED control.
Button inputs, knob/fader data, and all deck/touchstrip LEDs are HID on interface 5 —
there is no architectural reason the mixer LEDs would use a separate transport.

### KS8Facade investigation

The `Audio::DeviceControlling::KS8Facade` class implements `FacadeCSIBridge` and
`TraktorModeCoreCallback`. Its TraktorModeCoreCallback vtable's vFunc_1/vFunc_2 are simple
getters, not writers. The FacadeCSIBridge vtable has KS8-unique functions
(sub_1405dad10, sub_1405d09e0, sub_1405d1750, sub_1405c13a0, sub_1405c65e0) but these
appear to be property-system bridges, not direct USB writers. The KS8Facade bridges
CSI properties to the NHI/NHL2 layer; the actual USB writes happen lower down in NHL2.

## Current state

The full trace from CUE A button → USB packet has been completed via static analysis:

1. Property `mixer.channels.1.cue` → NHI ID `CueA` (0x43756541)
2. NHL2 controller (`sub_140996bc0`) dispatches to `sub_14099a3a0`
3. → `sub_14099ae10` (locks mutex, calls `sub_14099b120`)
4. → `sub_14099b120` sets LED slot 0x18 active
5. → `sub_140c9ba40(0x18, …, 1)` → absolute LED index 0x77 = 119
6. → `sub_140c9e870` → SIMPLE type, 0x81 packet, data offset 1
7. → `sub_1409a6410` → `sub_14079c370` writes byte to LED state buffer
8. → flush via `sub_140ca33e0` sends 309-byte interrupt OUT on endpoint 0x03

**The binary trace appears correct** — the static analysis chain is fully resolved and
consistently points to 0x81 prefix packets for CUE/filter LEDs. However, all practical
tests of 0x81 packets (including the FX assign offsets at 29–57 which are also binary-confirmed)
have produced zero mixer LED responses. The 0x82 range was also fully swept with no result.

The disconnect between a clean binary trace and zero hardware response is unexplained.
Current hypothesis: the 0xf3 "handshake" may not be sufficient, or there is an additional
initialization step required before the device accepts 0x80/0x81/0x82 LED packets for
the mixer section.

## Open questions

- Do ANY 0x80/0x81/0x82 offsets light mixer LEDs, even with handshake in same session?
  (mixer_sweep binary tests this comprehensively)
- Is the 0xf3 packet PREVENTING LED control (putting device in hardware mode)?
  → test `mixer_sweep --no-hs` to see if skipping 0xf3 allows 0x81 to work
- Is there a separate initialization packet beyond [0xf3, 0x01] that Traktor sends?
- Does the device require a specific interface alt-setting before accepting LED packets?

## Next steps

Run `mixer_sweep` (combined session, handshake then full sweep) and `mixer_sweep --no-hs`
to definitively determine whether ANY (prefix, offset) pair lights a mixer LED via HID.

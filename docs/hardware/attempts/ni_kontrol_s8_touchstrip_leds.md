# S8 Touchstrip LED Addressing — Attempts Log

The S8 has two touchstrips (left and right deck), each containing 25 bi-colour LEDs
(blue + orange/white). The D2 uses the same physical hardware and its touchstrip LED
addresses are known and working: blue = offsets 68–92, orange = offsets 93–117, within
the 309-byte LED buffer sent with prefix `0x80` to endpoint `0x03` on interface 5.

All attempts below failed to produce any visible effect on the touchstrip LEDs unless
otherwise noted.

---

## Hardware context

- **Interface:** 5, endpoint out `0x03` (interrupt), endpoint in `0x84`
- **LED buffer format:** `[prefix_byte] + [308 bytes data]` = 309 bytes total
- **Known working prefixes:** `0x80` (left deck), `0x81` (right deck), `0x82` (mixer)
- **D2 touchstrip offsets (known working on D2):** blue 68–92, orange 93–117

---

## Traktor decompile reference

Three reference documents exist from a Traktor binary decompile:
`docs/hardware/reference/ni_kontrol_s8_touchstrip.md`,
`ni_kontrol_s8_touchstrip_code.txt`, and
`ni_kontrol_s8_touchstrip_behavior.md`.

Key findings:

- `sub_141765d30` registers touchstrip LEDs with signature:
  `(port, name, start_index, step, hardware_id, type, count)`
- **Left strip:** `start=0x5d (93), step=2, hardware_id=0x44 (68), count=25`
  → buffer positions 93, 95, 97 … 141 (interleaved, every other byte)
- **Right strip:** `start=0xb7 (183), step=2, hardware_id=0x9e (158), count=25`
  → the reference states these fall in the `0x81` report at absolute positions
  `0xd3–0xeb` = **211–235**
- The 309-byte buffer is segmented across three reports:
  - `0x80`: bytes 0–117 (left deck, incl. left touchstrip)
  - `0x81`: bytes 118–235 (right deck, incl. right touchstrip at 211–235)
  - `0x82`: bytes 236–308 (mixer)

---

## Attempt 1 — D2 addresses directly (`touchstrip_d2_addresses.rs`)

Sent the D2 touchstrip offsets (68–92 blue, 93–117 orange) on the S8 using prefix `0x80`,
309-byte buffer, endpoint `0x03`.

**Tests run:**
- Single blue LED at offset 68
- Single orange LED at offset 93
- Both simultaneously
- Sweep of all 25 blue LEDs (68–92) at 100ms intervals

**Result:** No touchstrip response. Other LEDs at those same offsets may have responded
(pad/button LEDs share part of the buffer) but touchstrip remained dark.

---

## Attempt 2 — D2-style 123-byte buffer (`steyr_style_test.rs`)

Tried sending a shorter 123-byte buffer in the style used by the Steyr app for the D2,
rather than the full 309-byte S8 buffer.

**Tests run:**
1. 123-byte buffer, prefix `0x80`, orange range (offsets 93–117) = 255
2. 123-byte buffer, prefix `0x80`, blue range (offsets 68–92) = 255
3. 123-byte buffer, prefix `0x80`, both ranges = 255
4. 123-byte buffer, prefix `0x81` (right deck), orange = 255
5. Same as test 1 but on endpoint `0x01` — rejected by OS as expected
6. S5-style: send all three prefix reports (0x80, 0x81, 0x82) sequentially, 123 bytes each
7. 309-byte buffer, prefix `0x80`, pad 1 red (offset 0, confirmed lit) + orange (93–117)

**Result:** No touchstrip LEDs lit in any test. Test 7 confirmed pad 1 lit correctly as a
sanity check — the hardware was receiving and processing the LED data, but touchstrip
offsets remained unresponsive.

---

## Attempt 3 — Blit data accidentally sent to LED endpoint

During early screen testing, the screen blit header was accidentally sent to the LED
endpoint `0x03` (before the correct bulk endpoint `0x04` was identified). This appeared
to momentarily light pad 1 and the first touchstrip LED.

Subsequent attempts to replicate this were made in `blit_led_test.rs`:

**Tests run:**
1. 20-byte blit header via `interrupt_out` to `0x03`
2. 309-byte buffer starting with the blit header bytes
3. 309-byte buffer, prefix `0x80`, byte 18 = 0xFF (matching the `0xFF` position in the header)
4. 309-byte buffer, prefix `0x84` (blit header byte[0]), touchstrip offsets = 255
5. Full 261,148-byte screen blit via `bulk_out` to `0x03`
6. First 309 bytes of blit buffer via `interrupt_out`
7. 309-byte buffer with touchstrip offsets on interface 6 (screen), via `bulk_out` to `0x04`

**Result:** None of these replicated the accidental lighting. Conclusion: the original
observation was likely residual state from a prior LED test, not caused by the blit data.

---

## Attempt 4 — Bi-colour offset variant (`touchstrip_bicolor_test.rs`)

Hypothesis: the blue and orange LEDs might not be at D2 offsets 68/93 but at different
positions within the same buffer.

**Tests run:**
- Blue at offset 93, orange at offset 118 (93 + 25)
- Swapped: blue at 68, orange at 93 (D2 layout)
- Both simultaneously

**Result:** No response.

---

## Attempt 5 — Alternate prefixes (`touchstrip_prefix_test.rs`)

Hypothesis: the touchstrip might respond to a different prefix byte or be part of the
mixer group rather than the deck group.

**Tests run:**
- Prefix `0x81` (right deck), blue range 68–92, stepping one offset at a time
- Prefix `0x81` (right deck), orange range 93–117, stepping one offset at a time
- Prefix `0x82` (mixer), same ranges

**Result:** No touchstrip response on any prefix or offset in those ranges.

---

## Attempt 6 — Step-2 byte interleaving (`touchstrip_step_test.rs`)

Hypothesis: the 25 LEDs might not be packed consecutively but interleaved (every other
byte), similar to some NI protocols that alternate between two channels.

**Tests run:**
- Left deck: prefix `0x80`, starting at offset 93, stepping by 2 → offsets 93, 95, 97…141
  (individual LEDs one at a time)
- Right deck: prefix `0x81`, starting at offset 183, stepping by 2 → offsets 183, 185…231
  (individual LEDs one at a time)

**Result:** No response. Note: the step=2 interleaving is confirmed correct by the Traktor
decompile, but this test scanned individual LEDs rather than lighting all 25 simultaneously.
The combined effect may be needed. Additionally, the right strip absolute positions (211–235)
were never tested here.

---

## Attempt 7 — Full offset range scan (`touchstrip_high_offset_scan.rs`)

Hypothesis: offsets above 121 (beyond all known working LEDs) might contain touchstrip
addresses, and prefixes beyond `0x82` might select the touchstrip subsystem.

**Tests run:**
- All offsets 122–308 simultaneously, prefix `0x80` → no response
- Prefixes `0x83`–`0x8f`, offsets 68–117 simultaneously → no response on any prefix
- Prefixes `0x83`–`0x8f`, offsets 122–250 simultaneously → no response on any prefix

**Result:** No response anywhere in offsets 122–308 or on any prefix beyond `0x82`.

---

## What is confirmed working

- Pad LEDs (offsets 0–23, RGB, 3 bytes each) ✓
- All button/single LEDs (offsets 24–67) ✓
- Loop circle LEDs (offsets 60–67) ✓
- Deck selector LEDs (offsets 118–121) ✓
- Right deck (`0x81`) mirrors left deck addressing for all of the above ✓

---

## Attempt 8 — Decompile-derived targeted tests (`touchstrip_decompile_test.rs`)

From the Traktor decompile reference documents, three specific hypotheses were tested:

**Test A** — Left strip, all 25 interleaved simultaneously (step=2): prefix `0x80`, offsets
93, 95, 97…141, 309-byte buffer. **No response.**

**Test B** — Right strip at decompile-stated absolute offsets 211–235: prefix `0x81`,
flat and interleaved. **No response.**

**Test C** — Segmented send: `0x81` as a 119-byte report with right strip at relative
offsets 93–117. **RIGHT TOUCHSTRIP LIT (C1).** Also C2 (interleaved 93,95…141) lit
alternate orange LEDs.

**Key discovery:** The reports must be sent as **119-byte packets** (`[prefix] + [118 bytes]`),
not embedded in the full 309-byte LED buffer. The right strip uses prefix `0x81` with the
same relative offsets as the left strip uses with prefix `0x80`.

---

## Attempt 9 — Colour and brightness mapping (`touchstrip_blue_test.rs`)

With the 119-byte format confirmed, tested the full colour and brightness mapping.

**Results:**
- **Blue:** offsets 68–92, prefix `0x80` (left) / `0x81` (right) — **confirmed all 25 lit**
- **Orange:** offsets 93–117 — **confirmed all 25 lit**
- **Both simultaneously** → all LEDs show **purple** (additive mix) — confirmed bi-colour
- **Individual LED walk** — each offset 68–92 corresponds to one LED; note: hardware latches
  state (last written value persists until explicitly overwritten; sending an all-zero buffer
  does not reset LEDs that weren't explicitly included in that packet)
- **Brightness** — values 0–255 are analog (not binary on/off)
- **Right strip** — identical layout with prefix `0x81`

---

## SOLVED — Final addressing

| Channel | Prefix | Offsets   | Count |
|---------|--------|-----------|-------|
| Left blue   | `0x80` | 68–92  | 25    |
| Left orange | `0x80` | 93–117 | 25    |
| Right blue  | `0x81` | 68–92  | 25    |
| Right orange| `0x81` | 93–117 | 25    |

**Report format:** `[prefix_byte] + [118 data bytes]` = 119 bytes total, sent as an
interrupt_out on interface 5, endpoint `0x03`.

This is implemented in the descriptor as two LED groups (`left_touchstrip`, `right_touchstrip`)
with `buffer_size: 118`.

---

## Open hypotheses — priority order

### 1. All 25 interleaved LEDs simultaneously (highest priority)

The Traktor decompile confirms `step=2`. All previous tests either sent consecutive
offsets OR sent interleaved offsets one at a time. We have never sent all 25 interleaved
positions simultaneously:

- **Left:** prefix `0x80`, offsets 93, 95, 97, 99 … 141 (all 25 at once)
- **Right:** prefix `0x81`, offsets 211, 213, 215 … 259 or 183, 185 … 229 (both variants)

The hardware may require a minimum number of LEDs lit before it activates the touchstrip
display.

### 2. Right strip at Traktor-decompiled absolute offsets 211–235 via `0x81`

The reference document states right strip falls at `0xd3–0xeb` (211–235) within the
`0x81` report. We have never targeted these specific offsets with prefix `0x81`. This
is a direct, untested claim from the decompile.

### 3. Segmented send — each prefix covers only its own slice

All tests to date have sent a full 309-byte buffer and set bytes within it. Traktor may
segment differently: send 118 bytes with `0x80` (covering only the left half), then 118
bytes with `0x81` (covering only the right half, with offsets relative to the start of
that segment). The right touchstrip at absolute 211 = relative offset 93 within the
`0x81` segment — exactly mirroring the left strip's layout.

### 4. USB capture of Traktor

A Wireshark/usbmon capture of Traktor Pro driving the touchstrips would be definitive.
This has not been attempted.

#!/usr/bin/env python3
"""Analyze input address patterns to find D2 -> S8 LED address mapping"""

import json

# D2 inputs - key buttons and their addresses
d2_inputs = {
    "play": {"byte": 8, "mask": 0x10},
    "cue": {"byte": 8, "mask": 0x20},
    "sync": {"byte": 8, "mask": 0x40},
    "shift": {"byte": 8, "mask": 0x80},
    "hotcue": {"byte": 8, "mask": 0x08},
    "loop": {"byte": 8, "mask": 0x04},
    "freeze": {"byte": 8, "mask": 0x02},
    "remix": {"byte": 8, "mask": 0x01},
    "flux": {"byte": 7, "mask": 0x10},
    "deck": {"byte": 7, "mask": 0x04},
    "pad_1": {"byte": 7, "mask": 0x08},
    "pad_2": {"byte": 7, "mask": 0x01},
    "pad_3": {"byte": 6, "mask": 0x01},
    "pad_4": {"byte": 6, "mask": 0x02},
    "pad_5": {"byte": 7, "mask": 0x20},
    "pad_6": {"byte": 7, "mask": 0x40},
    "pad_7": {"byte": 7, "mask": 0x80},
    "pad_8": {"byte": 7, "mask": 0x02},
}

# D2 LEDs - known working addresses
d2_leds = {
    "play": 59,
    "cue": 58,
    "sync_green": 56,
    "sync_red": 57,
    "shift": 55,
    "hotcue_white": 44,
    "hotcue_blue": 45,
    "loop_white": 46,
    "loop_blue": 47,
    "freeze_white": 48,
    "freeze_blue": 49,
    "remix_white": 50,
    "remix_blue": 51,
    "flux": 52,
    "deck_white": 53,
    "deck_blue": 54,
    "pad_1": (2, 1, 0),      # (r, g, b)
    "pad_2": (5, 4, 3),
    "pad_3": (8, 7, 6),
    "pad_4": (11, 10, 9),
    "pad_5": (14, 13, 12),
    "pad_6": (17, 16, 15),
    "pad_7": (20, 19, 18),
    "pad_8": (23, 22, 21),
}

# S8 LEFT side inputs - from discovered data
s8_left_inputs = {
    "left_play": {"byte": 9, "mask": 0x10},
    "left_cue": {"byte": 9, "mask": 0x20},
    "left_sync": {"byte": 9, "mask": 0x40},
    "left_shift": {"byte": 9, "mask": 0x80},
    "left_hotcue": {"byte": 9, "mask": 0x08},
    "left_loop": {"byte": 9, "mask": 0x04},
    "left_freeze": {"byte": 9, "mask": 0x02},
    "left_remix": {"byte": 9, "mask": 0x01},
    "left_flux": {"byte": 8, "mask": 0x10},
    "left_deck": {"byte": 8, "mask": 0x04},
    "left_pad_1": {"byte": 8, "mask": 0x08},
    "left_pad_2": {"byte": 8, "mask": 0x01},
    "left_pad_3": {"byte": 7, "mask": 0x01},
    "left_pad_4": {"byte": 7, "mask": 0x02},
    "left_pad_5": {"byte": 8, "mask": 0x20},
    "left_pad_6": {"byte": 8, "mask": 0x40},
    "left_pad_7": {"byte": 8, "mask": 0x80},
    "left_pad_8": {"byte": 8, "mask": 0x02},
}

print("=" * 70)
print("BYTE OFFSET ANALYSIS: D2 vs S8 LEFT")
print("=" * 70)

# Analyze byte offset shifts
offsets = {}
for control in d2_inputs:
    if f"left_{control}" in s8_left_inputs:
        d2_byte = d2_inputs[control]["byte"]
        s8_byte = s8_left_inputs[f"left_{control}"]["byte"]
        shift = s8_byte - d2_byte

        if shift not in offsets:
            offsets[shift] = []
        offsets[shift].append(f"{control}: D2 byte {d2_byte} -> S8 byte {s8_byte}")

print("\nByte offset shifts found:")
for shift in sorted(offsets.keys()):
    print(f"\n  Shift +{shift}:")
    for item in offsets[shift]:
        print(f"    {item}")

# The most common shift should be the base offset
if offsets:
    most_common_shift = max(offsets, key=lambda k: len(offsets[k]))
    print(f"\nMost common shift: +{most_common_shift}")
    print(f"Used for {len(offsets[most_common_shift])} controls")

print("\n" + "=" * 70)
print("INFERRED LED MAPPING: D2 -> S8 LEFT (prefix 0x80)")
print("=" * 70)
print("\nIf byte offset pattern matches byte offset in LED control response,")
print("then LED addresses might directly correspond:\n")

for control, led_addr in d2_leds.items():
    if isinstance(led_addr, tuple):
        print(f"  {control}: RGB offsets {led_addr} (D2: pad RGB)")
    else:
        print(f"  {control}: offset {led_addr} (D2)")

print("\n" + "=" * 70)
print("HYPOTHESIS FOR TESTING")
print("=" * 70)
print("""
S8 LEFT DECK (prefix 0x80) LED mapping should be similar to D2 but potentially
offset based on the input byte pattern. Test these in sequence:

For single LEDs (press in this order to verify):
  left_play -> offset 59 (same as D2)
  left_cue -> offset 58 (same as D2)
  left_sync -> offset 56 or 57 (same as D2)
  left_shift -> offset 55 (same as D2)

For pad LEDs (RGB):
  left_pad_1 -> offsets (2, 1, 0) (same as D2 pad_1)
  left_pad_2 -> offsets (5, 4, 3) (same as D2 pad_2)

This assumes S8 LEFT uses the same LED address space as D2's single deck.
If this doesn't work, we may need to look at a completely different mapping.
""")

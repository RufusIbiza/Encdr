#!/usr/bin/env python3
"""Predict S8 LED addresses based on D2 mapping"""

import json

# Load both descriptors
with open('encdr/descriptors/ni_kontrol_d2.json') as f:
    d2 = json.load(f)

with open('encdr/descriptors/ni_kontrol_s8.json') as f:
    s8 = json.load(f)

print("╔════════════════════════════════════════════════════════════╗")
print("║     S8 LED Address Prediction Based on D2 Mapping         ║")
print("╚════════════════════════════════════════════════════════════╝\n")

# Extract D2 single LED addresses
d2_leds = {}
for item in d2['leds']['items']:
    if item['type'] == 'single':
        d2_leds[item['name']] = item['offset']
    elif item['type'] == 'strip':
        d2_leds[item['name']] = (item['offset'], item['count'])

print("D2 LED Addresses (reference):")
for name, addr in sorted(d2_leds.items(), key=lambda x: x[1] if isinstance(x[1], int) else x[1][0]):
    if isinstance(addr, tuple):
        print(f"  {name:25} offsets {addr[0]}-{addr[0]+addr[1]-1} ({addr[1]} LEDs)")
    else:
        print(f"  {name:25} offset {addr}")

print("\n" + "="*60)
print("Predicted S8 LEFT deck addresses (should match D2):\n")

# Check what's in S8 LEFT deck
left_deck = next(g for g in s8['leds'] if g['id'] == 'left_deck')
s8_singles = {}
for item in left_deck['items']:
    if item['type'] == 'single':
        s8_singles[item['name']] = item['offset']
    elif item['type'] == 'strip':
        s8_singles[item['name']] = (item['offset'], item['count'])

print("Current S8 LEFT deck mapping:")
for name in sorted(s8_singles.keys()):
    addr = s8_singles[name]
    if isinstance(addr, tuple):
        print(f"  {name:25} offsets {addr[0]}-{addr[0]+addr[1]-1} ({addr[1]} LEDs)")
    else:
        print(f"  {name:25} offset {addr}")

print("\n" + "="*60)
print("\nVerification of LED ranges:\n")

# Group by offset ranges
ranges = {
    "Pads": (0, 23),
    "FX buttons": (24, 28),
    "Screen buttons": (29, 36),
    "Control buttons": (37, 43),
    "Function buttons": (44, 67),
    "Touchstrip Blue": (68, 92),
    "Touchstrip Orange": (93, 117),
    "Deck selector": (118, 121),
}

for range_name, (start, end) in ranges.items():
    items_in_range = [name for name, addr in s8_singles.items()
                      if isinstance(addr, int) and start <= addr <= end]
    if items_in_range:
        print(f"  {range_name:20} (offsets {start:3}-{end:3}): {len(items_in_range)} LEDs")
        for item in sorted(items_in_range, key=lambda x: s8_singles[x]):
            print(f"    - {item:22} offset {s8_singles[item]}")
    else:
        print(f"  {range_name:20} (offsets {start:3}-{end:3}): NO LEDS")

print("\n" + "="*60)
print("\nSummary:\n")
print(f"  D2 has {len(d2_leds)} single LEDs + strips")
print(f"  S8 LEFT currently has {len(s8_singles)} items")
print(f"\n  ✓ Prediction: S8 LEFT deck should use D2 addresses 0-121")
print(f"  ✓ Prediction: S8 RIGHT deck should use same 0-121 (with prefix 0x81)")
print(f"  ? Unknown: S8 MIXER deck LED addresses (prefix 0x82)")

#!/usr/bin/env python3
"""Correct S8 pad RGB offsets - index is RED, +1 is GREEN, +2 is BLUE"""

import json

# Load the descriptor
with open('encdr/descriptors/ni_kontrol_s8.json', 'r') as f:
    desc = json.load(f)

# The pad indices from original descriptor: 0, 3, 6, 9, 12, 15, 18, 21
# These are the RED offsets. Green is +1, Blue is +2.
pad_offsets = [
    {"name": "pad_1", "offsets": {"r": 0, "g": 1, "b": 2}},
    {"name": "pad_2", "offsets": {"r": 3, "g": 4, "b": 5}},
    {"name": "pad_3", "offsets": {"r": 6, "g": 7, "b": 8}},
    {"name": "pad_4", "offsets": {"r": 9, "g": 10, "b": 11}},
    {"name": "pad_5", "offsets": {"r": 12, "g": 13, "b": 14}},
    {"name": "pad_6", "offsets": {"r": 15, "g": 16, "b": 17}},
    {"name": "pad_7", "offsets": {"r": 18, "g": 19, "b": 20}},
    {"name": "pad_8", "offsets": {"r": 21, "g": 22, "b": 23}},
]

# Create a mapping for quick lookup
pad_map = {p["name"]: p["offsets"] for p in pad_offsets}

# Fix each LED group
for led_group in desc['leds']:
    new_items = []
    for item in led_group['items']:
        if item['type'] == 'rgb' and item['name'] in pad_map:
            # Update with correct offsets
            item['offsets'] = pad_map[item['name']]
            print(f"{led_group['id']:12} {item['name']:20} -> {item['offsets']}")
        new_items.append(item)
    led_group['items'] = new_items

# Write back
with open('encdr/descriptors/ni_kontrol_s8.json', 'w') as f:
    json.dump(desc, f, indent=2)

print("\n✓ Corrected pad LED offsets (index is RED, +1 is GREEN, +2 is BLUE)")

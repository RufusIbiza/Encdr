#!/usr/bin/env python3
"""Restructure S8 LED descriptor into separate groups by prefix"""

import json

def index_to_rgb_offsets(index):
    """Convert RGB index to offsets format (like D2)"""
    base = index * 3
    return {
        "r": base + 2,
        "g": base + 1,
        "b": base + 0
    }

# Load the descriptor
with open('encdr/descriptors/ni_kontrol_s8.json', 'r') as f:
    desc = json.load(f)

# Separate LEDs by prefix
left_items = []
right_items = []
mixer_items = []

for item in desc['leds']['items']:
    # Convert index to offsets for RGB
    if item['type'] == 'rgb' and 'index' in item:
        offsets = index_to_rgb_offsets(item['index'])
        item = {
            'type': 'rgb',
            'name': item['name'],
            'offsets': offsets
        }

    # Route items to appropriate prefix group
    if 'left_' in item['name']:
        # Remove the 'left_' prefix for cleaner names
        clean_item = item.copy()
        clean_item['name'] = item['name'].replace('left_', '')
        left_items.append(clean_item)
    elif 'right_' in item['name']:
        # Remove the 'right_' prefix for cleaner names
        clean_item = item.copy()
        clean_item['name'] = item['name'].replace('right_', '')
        right_items.append(clean_item)
    elif 'mixer_' in item['name']:
        # Keep mixer names as-is (they're unique)
        mixer_items.append(item)

# Create new LED structure with separate groups
new_leds = [
    {
        "id": "left_deck",
        "interface": "control",
        "buffer_size": 309,
        "prefix_byte": "0x80",
        "items": left_items
    },
    {
        "id": "right_deck",
        "interface": "control",
        "buffer_size": 309,
        "prefix_byte": "0x81",
        "items": right_items
    },
    {
        "id": "mixer",
        "interface": "control",
        "buffer_size": 309,
        "prefix_byte": "0x82",
        "items": mixer_items
    }
]

# Replace the leds section
desc['leds'] = new_leds

# Write back
with open('encdr/descriptors/ni_kontrol_s8.json', 'w') as f:
    json.dump(desc, f, indent=2)

print(f"✓ Restructured LED groups:")
print(f"  LEFT (0x80):  {len(left_items)} items")
print(f"  RIGHT (0x81): {len(right_items)} items")
print(f"  MIXER (0x82): {len(mixer_items)} items")

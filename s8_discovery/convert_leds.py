#!/usr/bin/env python3
"""Convert S8 LED descriptor from index-based to offset-based format"""

import json

# Convert RGB index to offsets format
# For index N, the three bytes are at offsets: N*3, N*3+1, N*3+2
# Following D2 layout: r=offset+2, g=offset+1, b=offset
def index_to_rgb_offsets(index):
    base = index * 3
    return {
        "r": base + 2,
        "g": base + 1,
        "b": base + 0
    }

# Load the descriptor
with open('encdr/descriptors/ni_kontrol_s8.json', 'r') as f:
    desc = json.load(f)

# Convert RGB pads
new_items = []
for item in desc['leds']['items']:
    if item['type'] == 'rgb' and 'index' in item:
        # Convert from index-based to offsets-based
        new_item = {
            'type': 'rgb',
            'name': item['name'],
            'offsets': index_to_rgb_offsets(item['index'])
        }
        new_items.append(new_item)
        print(f"  {item['name']}: index {item['index']} -> offsets {new_item['offsets']}")
    else:
        # Keep as-is
        new_items.append(item)

# Replace the items
desc['leds']['items'] = new_items

# Write back
with open('encdr/descriptors/ni_kontrol_s8.json', 'w') as f:
    json.dump(desc, f, indent=2)

print(f"\n✓ Converted {len([i for i in new_items if i['type'] == 'rgb' and 'offsets' in i])} RGB LED items")

#!/usr/bin/env python3
"""Fix S8 LED descriptor with correct RGB pad mapping"""

import json

# Load the descriptor
with open('encdr/descriptors/ni_kontrol_s8.json', 'r') as f:
    desc = json.load(f)

# The correct mapping: index is the RED offset, add 1 for G, add 2 for B
def index_to_rgb_offsets(index):
    return {
        "r": index,
        "g": index + 1,
        "b": index + 2
    }

# Fix each LED group
for led_group in desc['leds']:
    new_items = []
    for item in led_group['items']:
        if item['type'] == 'rgb' and 'index' in item:
            # Convert from index-based to offsets-based
            new_item = {
                'type': 'rgb',
                'name': item['name'],
                'offsets': index_to_rgb_offsets(item['index'])
            }
            new_items.append(new_item)
            print(f"{led_group['id']:12} {item['name']:20} index {item['index']:3} -> offsets {new_item['offsets']}")
        else:
            new_items.append(item)

    led_group['items'] = new_items

# Write back
with open('encdr/descriptors/ni_kontrol_s8.json', 'w') as f:
    json.dump(desc, f, indent=2)

print("\n✓ Fixed RGB pad mappings in descriptor")

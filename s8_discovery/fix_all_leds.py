#!/usr/bin/env python3
"""Fix S8 LED descriptor using D2 as reference for LEFT/RIGHT decks"""

import json

# Load both descriptors
with open('encdr/descriptors/ni_kontrol_d2.json') as f:
    d2 = json.load(f)

with open('encdr/descriptors/ni_kontrol_s8.json') as f:
    s8 = json.load(f)

# Extract D2 LED items (excluding pads which S8 has its own mapping for)
d2_leds = []
for item in d2['leds']['items']:
    if item['type'] != 'rgb':  # Skip pads, we handle those separately
        d2_leds.append(item)

print("Building corrected S8 LED groups...\n")

# For LEFT and RIGHT decks, use D2 as reference
# Start with pads (0-23) then add D2 buttons
def build_deck_leds(deck_name, d2_items):
    """Build LED items for a deck using D2 as reference"""
    items = []

    # Add the 8 pads (0-23)
    for i in range(1, 9):
        r_offset = (i-1) * 3
        items.append({
            "type": "rgb",
            "name": f"pad_{i}",
            "offsets": {
                "r": r_offset,
                "g": r_offset + 1,
                "b": r_offset + 2
            }
        })

    # Add all the D2 buttons/singles/strips
    items.extend(d2_items)

    return items

# Build new LED groups
new_leds = []

# LEFT deck (0x80)
left_items = build_deck_leds("left", d2_leds)
new_leds.append({
    "id": "left_deck",
    "interface": "control",
    "buffer_size": 309,
    "prefix_byte": "0x80",
    "items": left_items
})

# RIGHT deck (0x81) - same structure as LEFT
right_items = build_deck_leds("right", d2_leds)
new_leds.append({
    "id": "right_deck",
    "interface": "control",
    "buffer_size": 309,
    "prefix_byte": "0x81",
    "items": right_items
})

# MIXER deck (0x82) - extract any mixer-specific LEDs from current descriptor
mixer_group = next(g for g in s8['leds'] if g['id'] == 'mixer')
new_leds.append({
    "id": "mixer",
    "interface": "control",
    "buffer_size": 309,
    "prefix_byte": "0x82",
    "items": mixer_group['items']  # Keep the existing mixer items
})

# Replace the leds section
s8['leds'] = new_leds

# Write back
with open('encdr/descriptors/ni_kontrol_s8.json', 'w') as f:
    json.dump(s8, f, indent=2)

print(f"✓ LEFT deck:  {len(left_items)} LED items (8 pads + {len(d2_leds)} buttons/strips)")
print(f"✓ RIGHT deck: {len(right_items)} LED items (8 pads + {len(d2_leds)} buttons/strips)")
print(f"✓ MIXER deck: {len(mixer_group['items'])} LED items")
print(f"\n✓ Descriptor fixed!")

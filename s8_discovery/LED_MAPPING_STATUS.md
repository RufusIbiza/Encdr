# S8 LED Mapping Status

## Summary

The S8 descriptor has been significantly improved with corrected LED mappings based on pattern analysis and empirical testing.

## What's Been Done

### ✓ Pads (Offsets 0-23)
**Status: VERIFIED AND WORKING**

The 8 RGB performance pads are correctly mapped:
- Pad 1: R=0, G=1, B=2 ✓ (tested and confirmed)
- Pad 2: R=3, G=4, B=5 ✓ (tested and confirmed)
- Pads 3-8: R=6+, following the same pattern ✓

Each pad uses 3 consecutive bytes for RGB values.

### ✓ Buttons & Controls (Offsets 24-121)
**Status: PREDICTED (based on D2 mapping)**

The S8 LEFT and RIGHT decks are predicted to follow the exact same LED addressing as the D2:

#### Button Groups:
- **Offsets 24-28**: FX buttons (fx_select, fx_1-4)
- **Offsets 29-36**: Screen buttons (4 left + 4 right)
- **Offsets 37-43**: Control buttons (back, capture, edit, on_1-4)
- **Offsets 44-67**: Function buttons (hotcue, loop, freeze, remix, flux, deck, shift, sync, cue, play, loop circles)
- **Offsets 60-67**: Loop circle indicators (8 LEDs, 2 colors each)
- **Offsets 68-92**: Touchstrip blue (25 LEDs)
- **Offsets 93-117**: Touchstrip orange (25 LEDs)
- **Offsets 118-121**: Deck selector (deck_a, deck_b, deck_c, deck_d)

**Rationale:** The S8 LEFT deck is structurally identical to the D2 (8 pads + buttons), so it should use the same LED addressing scheme.

### ? Mixer LEDs (0x82 prefix)
**Status: UNKNOWN**

The mixer deck (4 channel mixer with EQ, faders, crossfader) has unknown LED addresses.
Currently only 4 items are defined (mixer_cue_a/b/c/d).

## Next Steps

### To Verify LEFT/RIGHT Buttons:
Test a few key offsets to confirm the D2-based prediction:
1. **Offset 24**: Should be `fx_select` button
2. **Offset 37**: Should be `back` button
3. **Offset 59**: Should be `play` button
4. **Offset 68-70**: Should be first few touchstrip lights

### To Map Mixer LEDs:
- Identify which channel strip has LED feedback
- Test offsets starting from 0 with prefix 0x82
- Map all mixer controls (gain, EQ, faders, etc.)

## File Structure

The descriptor now has three LED groups:

```json
{
  "leds": [
    { "id": "left_deck",  "prefix_byte": "0x80", ... },
    { "id": "right_deck", "prefix_byte": "0x81", ... },
    { "id": "mixer",      "prefix_byte": "0x82", ... }
  ]
}
```

Each group can send LEDs independently using its prefix byte on endpoint 0x03.

## Testing Tool

Use the interactive mapper to test any offset:

```bash
./target/release/led_mapper
# Examples:
# 24L  - Test offset 24 on LEFT deck
# 59R  - Test offset 59 on RIGHT deck
# 0M   - Test offset 0 on MIXER
```

## Implementation Files

- `encdr/descriptors/ni_kontrol_s8.json` - Main descriptor
- `s8_discovery/src/bin/led_mapper.rs` - Interactive LED offset tester
- `s8_discovery/predict_s8_leds.py` - Prediction analysis
- `s8_discovery/fix_all_leds.py` - Descriptor generation script

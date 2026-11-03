# Maschine Mk3 screen encoder wrap-around report (Encdr 0.7.52)

## Summary

On a Maschine Mk3, continued rotation of a screen encoder could make the controlled parameter jump to the opposite endpoint. Physical tracing showed that the eight screen encoders report a direct absolute position on a 1,000-step ring (`0..999`). Encdr treated that position as a non-wrapping 16-bit value.

The fix adds a narrowly scoped `wrap1000` descriptor encoding and assigns it to all eight Maschine Mk3 screen encoders. It was verified with a parser regression test and on physical hardware in both directions.

## Environment

- macOS 14.7.6 on Apple Silicon
- Maschine Mk3, firmware 1.45
- USB VID/PID `17cc:1600`
- Encdr 0.7.52, base commit `c6446cc`
- Ableton Live bridge using relative parameter adjustments with endpoint clamping

## Physical evidence

Captured deltas during continued rotation included:

```text
normal forward:  +0.024, +0.023, +0.022
boundary:        -0.976
normal backward: -0.026, -0.038, -0.044
boundary:        +0.967
```

Another backward crossing produced `+0.987`. These values identify a roughly 1,000-position ring. The Ableton bridge independently clamps targets to `0.0..1.0`, locating the unwanted wrap in the Encdr event delta.

## Root cause

`screen_encoder_1` through `screen_encoder_8` used `"encoding": "signed16"`. `EncoderState::update_signed16()` performs direct subtraction and consequently interprets a small movement through the hardware's `0..999` boundary as nearly 1,000 positions in the opposite direction.

The initially considered `wrap16_wide` encoding was also unsuitable because it assumes a 65,536-step ring. A synthetic test passed under that assumption, but physical tracing disproved the assumed ring size.

Encdr already contains the required 1,000-step shortest-path arithmetic in `EncoderState::update_erp()`. The existing `erp` encoding cannot be reused directly because it first decodes two analog wiper taps; the Mk3 supplies an already-decoded little-endian position.

## Fix

1. Add `EncoderEncoding::Wrap1000` for a direct little-endian absolute position on a `0..999` ring.
2. Pass that value to the existing 1,000-step shortest-path calculation.
3. Change all eight Maschine Mk3 screen encoder descriptors from `signed16` to `wrap1000`.

The existing `scale: 1000.0` is unchanged. This does not alter `erp`, `signed16`, or `wrap16_wide` behavior for other devices.

## Tests

The parser regression test `maschine_mk3_screen_encoder_wraps_without_direction_reversal` loads the actual built-in Mk3 descriptor and verifies:

- `990 -> 7` emits `+0.017`, not `-0.983`;
- `7 -> 995` emits `-0.012`, not `+0.988`.

```text
cargo test -p encdr maschine_mk3_screen_encoder_wraps_without_direction_reversal --offline
test result: ok. 1 passed; 0 failed
```

The full library run had 100 passing tests. Two unrelated GPU-context tests failed because no GPU context was available in the isolated terminal environment; all descriptor, encoder, and parser tests passed.

## Physical validation

Passed on the connected Maschine Mk3. After rebuilding against the patch, both boundary directions behaved continuously. Continued rotation at a mapped parameter minimum or maximum stayed at that endpoint, with no jump to the opposite endpoint.

## Attachment

`encdr-0.7.52-maschine-mk3-encoder-wrap.patch` contains the minimal implementation, descriptor changes, and regression test.


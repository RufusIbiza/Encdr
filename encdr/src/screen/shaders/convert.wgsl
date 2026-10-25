// RGBA8888 -> BGR565 big-endian compute shader.
//
// Each invocation converts one pixel from the input RGBA buffer
// to a 16-bit BGR565 value written as two bytes (big-endian) to the output buffer.
//
// Bind group 0:
//   @binding(0) input  - RGBA pixels packed as u32 (one u32 per pixel)
//   @binding(1) output - BGR565 pixels packed as u32 (two pixels per u32)
//   @binding(2) params - { pixel_count, width, dither (0 / 1), pad }

struct Params {
    pixel_count: u32,
    width: u32,
    dither: u32,
    pad: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

var<private> BAYER: array<u32, 16> = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);

// 8-bit RGB -> 565 with an optional ordered 4x4 dither for pixel (x, y): floor(v / step + (2 * bayer + 1) / 32).
// Must match `quantize_565` in screen/mod.rs.
fn quantize565(r: u32, g: u32, b: u32, x: u32, y: u32, dither: u32) -> u32 {
    if (dither == 0u) {
        return ((r >> 3u) << 11u) | ((g >> 2u) << 5u) | (b >> 3u);
    }
    let t = 2u * BAYER[(y & 3u) * 4u + (x & 3u)] + 1u;
    let r5 = min((r * 4u + t) >> 5u, 31u);
    let g6 = min((g * 8u + t) >> 5u, 63u);
    let b5 = min((b * 4u + t) >> 5u, 31u);
    return (r5 << 11u) | (g6 << 5u) | b5;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Each thread processes 2 pixels (packing two 16-bit values into one u32)
    let pair_idx = gid.x;
    let px0 = pair_idx * 2u;
    let px1 = px0 + 1u;

    if (px0 >= params.pixel_count) {
        return;
    }

    // Convert pixel 0
    let rgba0 = input[px0];
    let r0 = (rgba0 >> 0u) & 0xFFu;
    let g0 = (rgba0 >> 8u) & 0xFFu;
    let b0 = (rgba0 >> 16u) & 0xFFu;

    let bgr565_0 = quantize565(r0, g0, b0, px0 % params.width, px0 / params.width, params.dither);

    // Big-endian byte swap for pixel 0
    let hi0 = (bgr565_0 >> 8u) & 0xFFu;
    let lo0 = bgr565_0 & 0xFFu;

    // Convert pixel 1 (or zero-fill if out of bounds)
    var hi1 = 0u;
    var lo1 = 0u;
    if (px1 < params.pixel_count) {
        let rgba1 = input[px1];
        let r1 = (rgba1 >> 0u) & 0xFFu;
        let g1 = (rgba1 >> 8u) & 0xFFu;
        let b1 = (rgba1 >> 16u) & 0xFFu;

        let bgr565_1 = quantize565(r1, g1, b1, px1 % params.width, px1 / params.width, params.dither);

        hi1 = (bgr565_1 >> 8u) & 0xFFu;
        lo1 = bgr565_1 & 0xFFu;
    }

    // Pack two BGR565 BE pixels into one u32: [hi0, lo0, hi1, lo1]
    output[pair_idx] = hi0 | (lo0 << 8u) | (hi1 << 16u) | (lo1 << 24u);
}

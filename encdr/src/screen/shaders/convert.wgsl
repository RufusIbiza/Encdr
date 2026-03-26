// RGBA8888 -> BGR565 big-endian compute shader.
//
// Each invocation converts one pixel from the input RGBA buffer
// to a 16-bit BGR565 value written as two bytes (big-endian) to the output buffer.
//
// Bind group 0:
//   @binding(0) input  - RGBA pixels packed as u32 (one u32 per pixel)
//   @binding(1) output - BGR565 pixels packed as u32 (two pixels per u32)
//   @binding(2) params - { pixel_count: u32 }

struct Params {
    pixel_count: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

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

    let r5_0 = (r0 >> 3u) & 0x1Fu;
    let g6_0 = (g0 >> 2u) & 0x3Fu;
    let b5_0 = (b0 >> 3u) & 0x1Fu;
    let bgr565_0 = (r5_0 << 11u) | (g6_0 << 5u) | b5_0;

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

        let r5_1 = (r1 >> 3u) & 0x1Fu;
        let g6_1 = (g1 >> 2u) & 0x3Fu;
        let b5_1 = (b1 >> 3u) & 0x1Fu;
        let bgr565_1 = (r5_1 << 11u) | (g6_1 << 5u) | b5_1;

        hi1 = (bgr565_1 >> 8u) & 0xFFu;
        lo1 = bgr565_1 & 0xFFu;
    }

    // Pack two BGR565 BE pixels into one u32: [hi0, lo0, hi1, lo1]
    output[pair_idx] = hi0 | (lo0 << 8u) | (hi1 << 16u) | (lo1 << 24u);
}

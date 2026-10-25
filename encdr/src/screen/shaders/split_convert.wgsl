// Split double-width RGBA8888 input into left and right BGR565 big-endian outputs.
//
// Each invocation processes 2 horizontal pixels in a given row.

struct SplitParams {
    total_width: u32,
    left_width: u32,
    right_width: u32,
    height: u32,
    dither: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<storage, read> input_rgba: array<u32>;
@group(0) @binding(1) var<storage, read_write> left_bgr565: array<u32>;
@group(0) @binding(2) var<storage, read_write> right_bgr565: array<u32>;
@group(0) @binding(3) var<uniform> params: SplitParams;

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

fn rgba_to_bgr565_be(rgba: u32, x: u32, y: u32) -> u32 {
    let r = (rgba >> 0u) & 0xFFu;
    let g = (rgba >> 8u) & 0xFFu;
    let b = (rgba >> 16u) & 0xFFu;

    let bgr565 = quantize565(r, g, b, x, y, params.dither);

    let hi = (bgr565 >> 8u) & 0xFFu;
    let lo = bgr565 & 0xFFu;
    return hi | (lo << 8u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_x = gid.x;
    let y = gid.y;
    let total_pairs = params.total_width / 2u;
    let left_pairs = params.left_width / 2u;
    let right_pairs = params.right_width / 2u;

    if (pair_x >= total_pairs || y >= params.height) {
        return;
    }

    let px0 = pair_x * 2u;
    let px1 = px0 + 1u;

    let rgba0 = input_rgba[y * params.total_width + px0];
    let rgba1 = input_rgba[y * params.total_width + px1];

    // dither phase is per screen (the right screen restarts at its own x = 0), like the CPU conversion of each half
    var lx = px0;
    if (pair_x >= left_pairs) {
        lx = px0 - params.left_width;
    }
    let p0 = rgba_to_bgr565_be(rgba0, lx, y);
    let p1 = rgba_to_bgr565_be(rgba1, lx + 1u, y);
    let packed = p0 | (p1 << 16u);

    if (pair_x < left_pairs) {
        left_bgr565[y * left_pairs + pair_x] = packed;
    } else {
        let right_pair_x = pair_x - left_pairs;
        if (right_pair_x < right_pairs) {
            right_bgr565[y * right_pairs + right_pair_x] = packed;
        }
    }
}

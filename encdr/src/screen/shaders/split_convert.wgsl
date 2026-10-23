// Split double-width RGBA8888 input into left and right BGR565 big-endian outputs.
//
// Each invocation processes 2 horizontal pixels in a given row.

struct SplitParams {
    total_width: u32,
    left_width: u32,
    right_width: u32,
    height: u32,
}

@group(0) @binding(0) var<storage, read> input_rgba: array<u32>;
@group(0) @binding(1) var<storage, read_write> left_bgr565: array<u32>;
@group(0) @binding(2) var<storage, read_write> right_bgr565: array<u32>;
@group(0) @binding(3) var<uniform> params: SplitParams;

fn rgba_to_bgr565_be(rgba: u32) -> u32 {
    let r = (rgba >> 0u) & 0xFFu;
    let g = (rgba >> 8u) & 0xFFu;
    let b = (rgba >> 16u) & 0xFFu;

    let r5 = (r >> 3u) & 0x1Fu;
    let g6 = (g >> 2u) & 0x3Fu;
    let b5 = (b >> 3u) & 0x1Fu;
    let bgr565 = (r5 << 11u) | (g6 << 5u) | b5;

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

    let p0 = rgba_to_bgr565_be(rgba0);
    let p1 = rgba_to_bgr565_be(rgba1);
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

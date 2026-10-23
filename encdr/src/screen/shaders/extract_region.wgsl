// GPU compute shader to extract a sub-rectangle of 16-bit packed BGR565BE pixels.
//
// Each thread processes 2 horizontal pixels (one packed u32).

struct ExtractParams {
    src_width: u32,
    rect_x: u32,
    rect_y: u32,
    rect_w: u32,
    rect_h: u32,
}

@group(0) @binding(0) var<storage, read> src_pixels: array<u32>;
@group(0) @binding(1) var<storage, read_write> dst_pixels: array<u32>;
@group(0) @binding(2) var<uniform> params: ExtractParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_x = gid.x;
    let ry = gid.y;
    let dst_pairs = (params.rect_w + 1u) / 2u;
    let src_pairs = params.src_width / 2u;

    if (pair_x >= dst_pairs || ry >= params.rect_h) {
        return;
    }

    let sy = params.rect_y + ry;
    let src_pair_x = (params.rect_x / 2u) + pair_x;

    dst_pixels[ry * dst_pairs + pair_x] = src_pixels[sy * src_pairs + src_pair_x];
}

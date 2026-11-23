// GPU compute shader for fused bounds-driven region extraction.
//
// Reads the dirty bounding box directly from the diff pass's VRAM buffer
// (zero CPU round-trip) and packs either the aligned dirty sub-rectangle or
// the full frame into `dst_pixels`. Each thread copies 2 horizontal pixels
// (one packed u32 of BGR565BE).

struct DiffBoundsRead {
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
    dirty_count: u32,
}

struct FusedExtractParams {
    width: u32,
    height: u32,
    x_align: u32,
    y_align: u32,
    force_full: u32,
    partial_supported: u32,
    full_threshold_pct: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> src_pixels: array<u32>;
@group(0) @binding(1) var<storage, read_write> dst_pixels: array<u32>;
@group(0) @binding(2) var<storage, read> bounds: DiffBoundsRead;
@group(0) @binding(3) var<uniform> params: FusedExtractParams;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_x = gid.x;
    let ry = gid.y;
    let src_pairs = params.width / 2u;

    var rx: u32 = 0u;
    var r_y: u32 = 0u;
    var rw: u32 = params.width;
    var rh: u32 = params.height;

    if (params.force_full == 0u) {
        if (bounds.dirty_count == 0u) {
            return;
        }
        if (params.partial_supported != 0u) {
            // Ensure even horizontal alignment so packed u32 pixel pairs stay aligned.
            let xa = max(params.x_align, 2u);
            let ya = max(params.y_align, 1u);
            let ax = (bounds.min_x / xa) * xa;
            let ay = (bounds.min_y / ya) * ya;
            let ar = min(((bounds.max_x + xa) / xa) * xa, params.width);
            let ab = min(((bounds.max_y + ya) / ya) * ya, params.height);
            let aw = ar - ax;
            let ah = ab - ay;

            if (aw * ah * 100u <= params.width * params.height * params.full_threshold_pct) {
                rx = ax;
                r_y = ay;
                rw = aw;
                rh = ah;
            }
        }
    }

    let dst_pairs = rw / 2u;
    if (pair_x >= dst_pairs || ry >= rh) {
        return;
    }

    let sy = r_y + ry;
    let src_pair_x = (rx / 2u) + pair_x;
    dst_pixels[ry * dst_pairs + pair_x] = src_pixels[sy * src_pairs + src_pair_x];
}

// GPU compute shader for dirty-rect diffing between current and previous frames.
//
// Compares BGR565BE pixels packed as u32 (2 pixels per u32) and atomically
// updates the bounding box of modified pixels.

struct DiffBounds {
    min_x: atomic<u32>,
    max_x: atomic<u32>,
    min_y: atomic<u32>,
    max_y: atomic<u32>,
    dirty_count: atomic<u32>,
}

struct DiffParams {
    width: u32,
    height: u32,
    pixel_count: u32,
}

@group(0) @binding(0) var<storage, read> curr_pixels: array<u32>;
@group(0) @binding(1) var<storage, read> prev_pixels: array<u32>;
@group(0) @binding(2) var<storage, read_write> bounds: DiffBounds;
@group(0) @binding(3) var<uniform> params: DiffParams;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_idx = gid.x;
    let total_pairs = (params.pixel_count + 1u) / 2u;
    if (pair_idx >= total_pairs) {
        return;
    }

    let c = curr_pixels[pair_idx];
    let p = prev_pixels[pair_idx];

    if (c != p) {
        let px0 = pair_idx * 2u;
        let px1 = px0 + 1u;

        if ((c & 0xFFFFu) != (p & 0xFFFFu)) {
            let x0 = px0 % params.width;
            let y0 = px0 / params.width;
            atomicMin(&bounds.min_x, x0);
            atomicMax(&bounds.max_x, x0);
            atomicMin(&bounds.min_y, y0);
            atomicMax(&bounds.max_y, y0);
            atomicAdd(&bounds.dirty_count, 1u);
        }

        if (px1 < params.pixel_count && ((c >> 16u) & 0xFFFFu) != ((p >> 16u) & 0xFFFFu)) {
            let x1 = px1 % params.width;
            let y1 = px1 / params.width;
            atomicMin(&bounds.min_x, x1);
            atomicMax(&bounds.max_x, x1);
            atomicMin(&bounds.min_y, y1);
            atomicMax(&bounds.max_y, y1);
            atomicAdd(&bounds.dirty_count, 1u);
        }
    }
}

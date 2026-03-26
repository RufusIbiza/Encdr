use std::sync::mpsc;

use webkit2gtk::WebViewExt;
use webkit2gtk::{SnapshotOptions, SnapshotRegion};

/// Capture the current WebView contents as RGBA pixel data at the specified dimensions.
///
/// Uses WebKitGTK's native `get_snapshot()` which composites the full
/// page (HTML, CSS, Canvas, SVG — everything) through the GPU compositor.
/// Works on both X11 and Wayland.
///
/// If the snapshot is larger than `target_width x target_height` (e.g. due to
/// HiDPI scaling), it is scaled down via Cairo to match the target size exactly.
///
/// Returns `(width, height, rgba_pixels)` or an error.
pub fn capture_webview_pixels(
    webview: &webkit2gtk::WebView,
    target_width: u32,
    target_height: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let (tx, rx) = mpsc::channel();

    webview.snapshot(
        SnapshotRegion::FullDocument,
        SnapshotOptions::NONE,
        None::<&gtk::gio::Cancellable>,
        move |result| {
            tx.send(result).ok();
        },
    );

    // Pump the GTK main loop until the snapshot callback fires.
    // gtk::main_iteration_do(false) returns true when there are no events pending,
    // which is normal without a running main loop — it does NOT mean "quit".
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match rx.try_recv() {
            Ok(result) => {
                let surface = result.map_err(|e| format!("WebKit snapshot failed: {}", e))?;
                return extract_rgba_from_surface(&surface, target_width, target_height);
            }
            Err(mpsc::TryRecvError::Empty) => {
                if std::time::Instant::now() > deadline {
                    return Err("Snapshot timed out after 5 seconds".to_string());
                }
                // Pump pending GTK events
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("Snapshot callback channel disconnected".to_string());
            }
        }
    }
}

/// Extract RGBA pixel data from a Cairo surface, scaled to target dimensions.
///
/// The snapshot surface may be any type (GL, image, etc.). We render it
/// onto a fresh ARGB32 ImageSurface at the target size, scaling if needed
/// (e.g. HiDPI snapshots are larger than the logical WebView size).
/// Cairo stores ARGB32 as premultiplied BGRA on little-endian — we
/// convert to standard RGBA8888.
fn extract_rgba_from_surface(
    surface: &cairo::Surface,
    target_width: u32,
    target_height: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    // Get dimensions from the surface extents
    let cr_temp = cairo::Context::new(surface)
        .map_err(|e| format!("Failed to create context: {}", e))?;
    let extents = cr_temp.clip_extents().map_err(|e| format!("clip_extents: {}", e))?;
    let src_width = extents.2.ceil() as i32;
    let src_height = extents.3.ceil() as i32;

    if src_width <= 0 || src_height <= 0 {
        return Err(format!("Invalid surface dimensions: {}x{}", src_width, src_height));
    }

    let w = target_width;
    let h = target_height;

    // Create an ImageSurface at the TARGET size and paint the snapshot scaled to fit
    let mut image = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32)
        .map_err(|e| format!("Failed to create ImageSurface: {}", e))?;

    {
        let cr = cairo::Context::new(&image)
            .map_err(|e| format!("Failed to create context: {}", e))?;

        // Scale from snapshot size to target size if they differ
        if src_width != w as i32 || src_height != h as i32 {
            let sx = w as f64 / src_width as f64;
            let sy = h as f64 / src_height as f64;
            cr.scale(sx, sy);
        }

        cr.set_source_surface(surface, 0.0, 0.0)
            .map_err(|e| format!("set_source_surface: {}", e))?;
        cr.paint().map_err(|e| format!("paint: {}", e))?;
    }

    // Now read the pixel data from our owned ImageSurface
    let stride = image.stride() as u32;

    let data = image
        .data()
        .map_err(|e| format!("Failed to access surface data: {}", e))?;

    let mut rgba = Vec::with_capacity((w * h * 4) as usize);

    for y in 0..h {
        for x in 0..w {
            let offset = (y * stride + x * 4) as usize;
            if offset + 3 >= data.len() {
                rgba.extend_from_slice(&[0, 0, 0, 255]);
                continue;
            }

            // Cairo ARGB32 on little-endian is stored as BGRA in memory
            let b = data[offset];
            let g = data[offset + 1];
            let r = data[offset + 2];
            let a = data[offset + 3];

            // Un-premultiply alpha
            let (r, g, b) = if a == 0 {
                (0, 0, 0)
            } else if a == 255 {
                (r, g, b)
            } else {
                let a_f = a as f32 / 255.0;
                (
                    (r as f32 / a_f).min(255.0) as u8,
                    (g as f32 / a_f).min(255.0) as u8,
                    (b as f32 / a_f).min(255.0) as u8,
                )
            };

            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(a);
        }
    }

    Ok((w, h, rgba))
}

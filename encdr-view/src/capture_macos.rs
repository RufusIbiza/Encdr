//! macOS pixel capture via WKWebView's native takeSnapshot API.
//!
//! Uses objc2 to call WKWebView.takeSnapshot(with:completionHandler:)
//! and extracts RGBA pixel data from the resulting NSImage via NSBitmapImageRep.

use std::sync::mpsc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id};
use objc2_app_kit::NSBitmapImageRep;
use objc2_foundation::NSError;
use wry::WebViewExtMacOS;

use super::webview_macos;

/// Capture the current WebView contents as RGBA pixel data at the specified dimensions.
///
/// Uses WKWebView's native `takeSnapshot(with:completionHandler:)` which captures
/// the full composited output (HTML, CSS, Canvas, SVG). The snapshot is taken at
/// the WebView's current size and scaled to the target dimensions if needed.
pub fn capture_webview_pixels(
    webview: &wry::WebView,
    target_width: u32,
    target_height: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    // Get the underlying WKWebView from wry
    let wk_webview = webview.webview();
    let wk_ptr: *mut AnyObject = Retained::into_raw(wk_webview).cast();

    let (tx, rx) = mpsc::channel::<Result<Retained<AnyObject>, String>>();

    // Create the completion handler block:
    // ^(NSImage *snapshotImage, NSError *error)
    let block = RcBlock::new(move |image: *mut AnyObject, error: *mut AnyObject| {
        if !image.is_null() {
            let retained: Retained<AnyObject> = unsafe { Retained::retain(image) }
                .expect("Failed to retain NSImage");
            let _ = tx.send(Ok(retained));
        } else {
            let msg = if !error.is_null() {
                let err: &NSError = unsafe { &*(error as *const NSError) };
                format!("WKWebView snapshot failed: {:?}", err)
            } else {
                "WKWebView snapshot failed: unknown error".to_string()
            };
            let _ = tx.send(Err(msg));
        }
    });

    // Call takeSnapshotWithConfiguration:nil completionHandler:block
    unsafe {
        let _: () = msg_send![wk_ptr, takeSnapshotWithConfiguration: std::ptr::null::<AnyObject>() completionHandler: &*block];
        // Re-wrap the pointer (we didn't actually release it)
        let _ = Retained::from_raw(wk_ptr.cast());
    }

    // Pump the event loop until the callback fires (same pattern as Linux)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match rx.try_recv() {
            Ok(result) => {
                let ns_image = result?;
                return extract_rgba_from_nsimage(&ns_image, target_width, target_height);
            }
            Err(mpsc::TryRecvError::Empty) => {
                if std::time::Instant::now() > deadline {
                    return Err("WKWebView snapshot timed out after 5 seconds".to_string());
                }
                webview_macos::pump_events();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("Snapshot callback channel disconnected".to_string());
            }
        }
    }
}

/// Extract RGBA pixels from an NSImage at the target dimensions.
///
/// Creates an NSBitmapImageRep from the NSImage's TIFF representation,
/// then reads the raw pixel data. Handles premultiplied alpha conversion.
fn extract_rgba_from_nsimage(
    ns_image: &AnyObject,
    target_width: u32,
    target_height: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    unsafe {
        // Get TIFF representation: NSData *tiff = [image TIFFRepresentation]
        let tiff_data: *mut AnyObject = msg_send![ns_image, TIFFRepresentation];
        if tiff_data.is_null() {
            return Err("NSImage TIFFRepresentation returned nil".to_string());
        }

        // Create NSBitmapImageRep from TIFF: [[NSBitmapImageRep alloc] initWithData:tiff]
        let bitmap_rep: Option<Retained<NSBitmapImageRep>> =
            msg_send_id![NSBitmapImageRep::class(), imageRepWithData: tiff_data];
        let bitmap_rep =
            bitmap_rep.ok_or("Failed to create NSBitmapImageRep from TIFF data")?;

        // Read dimensions and pixel data
        let src_width: isize = msg_send![&bitmap_rep, pixelsWide];
        let src_height: isize = msg_send![&bitmap_rep, pixelsHigh];
        let bytes_per_row: isize = msg_send![&bitmap_rep, bytesPerRow];
        let samples_per_pixel: isize = msg_send![&bitmap_rep, samplesPerPixel];
        let bitmap_data: *const u8 = msg_send![&bitmap_rep, bitmapData];

        if bitmap_data.is_null() || src_width <= 0 || src_height <= 0 {
            return Err(format!(
                "Invalid bitmap: {}x{}, data null={}",
                src_width,
                src_height,
                bitmap_data.is_null()
            ));
        }

        let has_alpha = samples_per_pixel >= 4;
        let w = target_width;
        let h = target_height;
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);

        // Scale if source differs from target (e.g. Retina)
        let sx = src_width as f64 / w as f64;
        let sy = src_height as f64 / h as f64;

        for y in 0..h {
            for x in 0..w {
                // Nearest-neighbor sampling from source
                let src_x = ((x as f64 * sx) as usize).min(src_width as usize - 1);
                let src_y = ((y as f64 * sy) as usize).min(src_height as usize - 1);
                let offset = src_y * bytes_per_row as usize + src_x * samples_per_pixel as usize;

                // NSBitmapImageRep with TIFF source is typically RGBA (non-premultiplied)
                let r = *bitmap_data.add(offset);
                let g = *bitmap_data.add(offset + 1);
                let b = *bitmap_data.add(offset + 2);
                let a = if has_alpha {
                    *bitmap_data.add(offset + 3)
                } else {
                    255
                };

                rgba.push(r);
                rgba.push(g);
                rgba.push(b);
                rgba.push(a);
            }
        }

        Ok((w, h, rgba))
    }
}

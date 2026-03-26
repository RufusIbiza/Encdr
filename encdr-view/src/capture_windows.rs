//! Windows pixel capture via WebView2's CapturePreview API.
//!
//! Captures the WebView2 output as a PNG via ICoreWebView2::CapturePreview,
//! then decodes the PNG to RGBA pixel data using the `png` crate.

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::sync::mpsc;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2CapturePreviewCompletedHandler,
    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
};
use windows::core::implement;
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::System::Com::{IStream, STGC_DEFAULT};
use wry::WebViewExtWindows;

use super::webview_windows;

/// Capture the current WebView contents as RGBA pixel data at the specified dimensions.
///
/// Uses WebView2's CapturePreview to render the page to a PNG, then decodes to RGBA.
/// The PNG is rendered at the WebView's current size — if that differs from the target
/// dimensions, the decoded image is scaled via nearest-neighbor sampling.
pub fn capture_webview_pixels(
    webview: &wry::WebView,
    target_width: u32,
    target_height: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    unsafe {
        // Get the ICoreWebView2Controller → ICoreWebView2
        let controller = webview.controller();
        let core_webview: ICoreWebView2 = controller
            .CoreWebView2()
            .map_err(|e| format!("Failed to get CoreWebView2: {}", e))?;

        // Create an in-memory IStream for the PNG output
        let stream: IStream = CreateStreamOnHGlobal(None, true)
            .map_err(|e| format!("CreateStreamOnHGlobal failed: {}", e))?;

        // Set up completion handler
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        let handler = CaptureHandler::new(tx);

        // Initiate capture
        core_webview
            .CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler.into(),
            )
            .map_err(|e| format!("CapturePreview failed: {}", e))?;

        // Pump messages until the callback fires
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match rx.try_recv() {
                Ok(result) => {
                    result?;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    if std::time::Instant::now() > deadline {
                        return Err("CapturePreview timed out after 5 seconds".to_string());
                    }
                    webview_windows::pump_events();
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("CapturePreview callback channel disconnected".to_string());
                }
            }
        }

        // Read PNG bytes from the IStream
        let png_bytes = read_istream(&stream)?;

        // Decode PNG to RGBA
        decode_png_to_rgba(&png_bytes, target_width, target_height)
    }
}

/// Read all bytes from an IStream, seeking to the beginning first.
unsafe fn read_istream(stream: &IStream) -> Result<Vec<u8>, String> {
    // Seek to start
    let mut new_pos = 0u64;
    stream
        .Seek(0, 0 /* STREAM_SEEK_SET */, Some(&mut new_pos))
        .map_err(|e| format!("IStream Seek failed: {}", e))?;

    // Read in chunks
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let mut bytes_read = 0u32;
        stream
            .Read(
                chunk.as_mut_ptr() as *mut _,
                chunk.len() as u32,
                Some(&mut bytes_read),
            )
            .map_err(|e| format!("IStream Read failed: {}", e))?;

        if bytes_read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..bytes_read as usize]);
    }

    Ok(buf)
}

/// Decode PNG bytes to RGBA pixel data, scaling to target dimensions if needed.
fn decode_png_to_rgba(
    png_bytes: &[u8],
    target_width: u32,
    target_height: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(Cursor::new(png_bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode failed: {}", e))?;

    let info = reader.info();
    let src_width = info.width;
    let src_height = info.height;
    let color_type = info.color_type;

    let mut src_buf = vec![0u8; reader.output_buffer_size()];
    reader
        .next_frame(&mut src_buf)
        .map_err(|e| format!("PNG frame read failed: {}", e))?;

    // Convert to RGBA if needed
    let src_rgba = match color_type {
        png::ColorType::Rgba => src_buf,
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((src_width * src_height * 4) as usize);
            for chunk in src_buf.chunks_exact(3) {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(chunk[2]);
                rgba.push(255);
            }
            rgba
        }
        other => {
            return Err(format!("Unsupported PNG color type: {:?}", other));
        }
    };

    // Scale to target dimensions if needed
    if src_width == target_width && src_height == target_height {
        return Ok((target_width, target_height, src_rgba));
    }

    let sx = src_width as f64 / target_width as f64;
    let sy = src_height as f64 / target_height as f64;
    let mut rgba = Vec::with_capacity((target_width * target_height * 4) as usize);

    for y in 0..target_height {
        for x in 0..target_width {
            let src_x = ((x as f64 * sx) as u32).min(src_width - 1);
            let src_y = ((y as f64 * sy) as u32).min(src_height - 1);
            let offset = ((src_y * src_width + src_x) * 4) as usize;

            rgba.push(src_rgba[offset]);
            rgba.push(src_rgba[offset + 1]);
            rgba.push(src_rgba[offset + 2]);
            rgba.push(src_rgba[offset + 3]);
        }
    }

    Ok((target_width, target_height, rgba))
}

/// COM implementation of ICoreWebView2CapturePreviewCompletedHandler.
#[implement(ICoreWebView2CapturePreviewCompletedHandler)]
struct CaptureHandler {
    tx: mpsc::Sender<Result<(), String>>,
}

impl CaptureHandler {
    fn new(tx: mpsc::Sender<Result<(), String>>) -> Self {
        Self { tx }
    }
}

impl ICoreWebView2CapturePreviewCompletedHandler_Impl for CaptureHandler_Impl {
    fn Invoke(&self, errorcode: windows::core::HRESULT) -> windows::core::Result<()> {
        if errorcode.is_ok() {
            let _ = self.tx.send(Ok(()));
        } else {
            let _ = self.tx.send(Err(format!(
                "CapturePreview completed with error: 0x{:08x}",
                errorcode.0
            )));
        }
        Ok(())
    }
}

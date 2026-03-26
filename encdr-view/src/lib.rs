//! # encdr-view
//!
//! WebView-based screen renderer for hardware controller screens.
//!
//! Renders HTML/CSS/Canvas content via an offscreen WebView (wry + WebKitGTK),
//! captures the rendered pixels, and feeds them through encdr's GPU pipeline
//! for format conversion, frame diffing, and USB transfer.

pub mod bridge;
#[cfg(target_os = "linux")]
pub mod capture;
#[cfg(target_os = "linux")]
pub mod webview;

use encdr::core::descriptor::PixelFormat;
use encdr::core::event::DeviceId;
use encdr::Encdr;
use serde_json::Value;

/// How to load content into a ScreenView.
pub enum ScreenContent {
    /// Inline HTML string.
    Html(String),
    /// Path to an HTML file on disk.
    File(String),
}

impl ScreenContent {
    fn to_html(&self) -> Result<String, String> {
        match self {
            ScreenContent::Html(html) => Ok(html.clone()),
            ScreenContent::File(path) => {
                std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
            }
        }
    }
}

/// A WebView-backed screen renderer for a single device screen.
///
/// Manages an offscreen WebView sized to the device screen's native resolution.
/// The app pushes state updates via `send()`, the HTML/CSS/JS renders the UI,
/// and `ScreenView` automatically captures pixels and submits them through
/// encdr's GPU pipeline → frame diff → USB transfer.
pub struct ScreenView {
    #[cfg(target_os = "linux")]
    inner: webview::ManagedWebView,
    device_id: DeviceId,
    screen_name: String,
}

impl ScreenView {
    /// Create a new WebView-backed screen for a connected device.
    ///
    /// The WebView is sized to match the device screen's native resolution.
    /// If `visible` is true, the GTK window is shown on the desktop (useful
    /// for debugging). Otherwise it renders headless off-screen.
    pub fn new(
        encdr: &Encdr,
        device_id: DeviceId,
        screen_name: &str,
        content: ScreenContent,
        visible: bool,
    ) -> Result<Self, String> {
        // Look up the screen descriptor to get dimensions
        let descriptor = encdr
            .device_descriptor(device_id)
            .ok_or_else(|| format!("Device {:?} not connected", device_id))?;

        let screen_desc = descriptor
            .screens
            .iter()
            .find(|s| s.name == screen_name)
            .ok_or_else(|| format!("Screen '{}' not found on device", screen_name))?;

        let width = screen_desc.width as u32;
        let height = screen_desc.height as u32;
        let html = content.to_html()?;

        #[cfg(target_os = "linux")]
        {
            let inner = webview::ManagedWebView::new(width, height, &html, visible)?;

            Ok(Self {
                inner,
                device_id,
                screen_name: screen_name.to_string(),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (width, height, html);
            Err("encdr-view currently only supports Linux (WebKitGTK)".to_string())
        }
    }

    /// Push a state update to the WebView.
    ///
    /// Calls `window.encdr.onMessage(channel, data)` in the WebView's JS context.
    /// After the DOM updates, the bridge automatically requests a pixel capture
    /// on the next animation frame.
    pub fn send(&self, channel: &str, data: Value) {
        let js = bridge::build_send_js(channel, &data);

        #[cfg(target_os = "linux")]
        {
            if let Err(e) = self.inner.eval(&js) {
                tracing::warn!("Failed to send to WebView: {}", e);
            }
        }
    }

    /// Capture the current WebView contents and submit to encdr for USB transfer.
    ///
    /// This is called automatically when the WebView signals a dirty frame,
    /// but can also be called manually to force a capture.
    pub fn capture_and_submit(&self, encdr: &Encdr) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let (_w, _h, rgba) = capture::capture_webview_pixels(
                &self.inner.webkit_view,
                self.inner.width,
                self.inner.height,
            )?;

            encdr.submit_screen_with_format(
                self.device_id,
                &self.screen_name,
                &rgba,
                PixelFormat::Rgba8888,
            );

            self.inner.clear_frame_ready();
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = encdr;
            Err("Capture not supported on this platform".to_string())
        }
    }

    /// Check if the WebView has signaled that new content is ready for capture.
    pub fn is_frame_ready(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.inner.is_frame_ready()
        }

        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Process pending frames: if the WebView has rendered new content,
    /// capture and submit it to encdr.
    ///
    /// Call this in your main loop. It's a no-op if no new frame is ready.
    pub fn poll(&self, encdr: &Encdr) {
        if self.is_frame_ready() {
            if let Err(e) = self.capture_and_submit(encdr) {
                tracing::warn!("Frame capture failed: {}", e);
            }
        }
    }

    /// Pump the GTK event loop (Linux only).
    ///
    /// Must be called periodically to allow the WebView to process events
    /// and render. Returns `true` if GTK wants to quit.
    pub fn pump_events() -> bool {
        #[cfg(target_os = "linux")]
        {
            gtk::main_iteration_do(false)
        }

        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Load new HTML content into the WebView, replacing the current page.
    pub fn load_html(&self, html: &str) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            self.inner.load_html(html)
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = html;
            Err("Not supported on this platform".to_string())
        }
    }

    /// Execute arbitrary JavaScript in the WebView.
    pub fn eval(&self, js: &str) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            self.inner.eval(js)
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = js;
            Err("Not supported on this platform".to_string())
        }
    }

    /// Get the device ID this view is attached to.
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    /// Get the screen name this view renders to.
    pub fn screen_name(&self) -> &str {
        &self.screen_name
    }
}

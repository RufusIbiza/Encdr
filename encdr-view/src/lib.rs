//! # encdr-view
//!
//! WebView-based screen renderer for hardware controller screens.
//!
//! Renders HTML/CSS/Canvas content via an offscreen WebView, captures the
//! rendered pixels, and feeds them through encdr's GPU pipeline for format
//! conversion, frame diffing, and USB transfer.
//!
//! Supported platforms:
//! - **Linux**: GTK + WebKitGTK snapshot capture
//! - **macOS**: tao + wry + WKWebView `takeSnapshot` capture
//! - **Windows**: tao + wry + WebView2 `CapturePreview` capture

pub mod bridge;

#[cfg(target_os = "linux")]
pub mod capture;
#[cfg(target_os = "linux")]
pub mod webview;

#[cfg(target_os = "macos")]
pub mod capture_macos;
#[cfg(target_os = "macos")]
pub mod webview_macos;

#[cfg(target_os = "windows")]
pub mod capture_windows;
#[cfg(target_os = "windows")]
pub mod webview_windows;

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
    #[cfg(target_os = "macos")]
    inner: webview_macos::ManagedWebView,
    #[cfg(target_os = "windows")]
    inner: webview_windows::ManagedWebView,
    device_id: DeviceId,
    screen_name: String,
}

impl ScreenView {
    /// Create a new WebView-backed screen for a connected device.
    ///
    /// The WebView is sized to match the device screen's native resolution.
    /// If `visible` is true, a desktop window is shown (useful for debugging).
    /// Otherwise it renders headless off-screen.
    pub fn new(
        encdr: &Encdr,
        device_id: DeviceId,
        screen_name: &str,
        content: ScreenContent,
        visible: bool,
    ) -> Result<Self, String> {
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
        let inner = webview::ManagedWebView::new(width, height, &html, visible)?;

        #[cfg(target_os = "macos")]
        let inner = webview_macos::ManagedWebView::new(width, height, &html, visible)?;

        #[cfg(target_os = "windows")]
        let inner = webview_windows::ManagedWebView::new(width, height, &html, visible)?;

        Ok(Self {
            inner,
            device_id,
            screen_name: screen_name.to_string(),
        })
    }

    /// Push a state update to the WebView.
    ///
    /// Calls `window.encdr.onMessage(channel, data)` in the WebView's JS context.
    pub fn send(&self, channel: &str, data: Value) {
        let js = bridge::build_send_js(channel, &data);
        if let Err(e) = self.inner.eval(&js) {
            tracing::warn!("Failed to send to WebView: {}", e);
        }
    }

    /// Capture the current WebView contents and submit to encdr for USB transfer.
    pub fn capture_and_submit(&self, encdr: &Encdr) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        let (_w, _h, rgba) = capture::capture_webview_pixels(
            &self.inner.webkit_view,
            self.inner.width,
            self.inner.height,
        )?;

        #[cfg(target_os = "macos")]
        let (_w, _h, rgba) = capture_macos::capture_webview_pixels(
            &self.inner.webview,
            self.inner.width,
            self.inner.height,
        )?;

        #[cfg(target_os = "windows")]
        let (_w, _h, rgba) = capture_windows::capture_webview_pixels(
            &self.inner.webview,
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

    /// Check if the WebView has signaled that new content is ready for capture.
    pub fn is_frame_ready(&self) -> bool {
        self.inner.is_frame_ready()
    }

    /// Process pending frames: if the WebView has rendered new content,
    /// capture and submit it to encdr.
    pub fn poll(&self, encdr: &Encdr) {
        if self.is_frame_ready() {
            if let Err(e) = self.capture_and_submit(encdr) {
                tracing::warn!("Frame capture failed: {}", e);
            }
        }
    }

    /// Pump the platform event loop.
    ///
    /// Must be called periodically to allow the WebView to process events
    /// and render. On Linux this drives GTK, on macOS/Windows it drives
    /// the tao event loop.
    pub fn pump_events() -> bool {
        #[cfg(target_os = "linux")]
        {
            gtk::main_iteration_do(false)
        }

        #[cfg(target_os = "macos")]
        {
            webview_macos::pump_events();
            false
        }

        #[cfg(target_os = "windows")]
        {
            webview_windows::pump_events();
            false
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }

    /// Load new HTML content into the WebView, replacing the current page.
    pub fn load_html(&self, html: &str) -> Result<(), String> {
        self.inner.load_html(html)
    }

    /// Execute arbitrary JavaScript in the WebView.
    pub fn eval(&self, js: &str) -> Result<(), String> {
        self.inner.eval(js)
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

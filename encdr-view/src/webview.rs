use std::sync::{Arc, Mutex};

use gtk::prelude::*;
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix};

use crate::bridge;

/// Internal state for a managed WebView instance.
pub struct ManagedWebView {
    /// The wry WebView handle
    pub webview: WebView,
    /// The underlying WebKitGTK widget (for snapshot capture)
    pub webkit_view: webkit2gtk::WebView,
    /// GTK window container (kept alive)
    _window: gtk::Window,
    /// Screen dimensions
    pub width: u32,
    pub height: u32,
    /// Shared flag: set by IPC when JS signals a dirty frame
    pub frame_ready: Arc<Mutex<bool>>,
}

impl ManagedWebView {
    /// Create a new offscreen WebView sized to the device screen dimensions.
    ///
    /// Creates a small, undecorated GTK window positioned off-screen. The
    /// window must be shown (realized) for WebKitGTK to actually render —
    /// a hidden window produces no pixels.
    pub fn new(
        width: u32,
        height: u32,
        html: &str,
        visible: bool,
    ) -> Result<Self, String> {
        // Ensure GTK is initialized
        if gtk::init().is_err() {
            return Err("Failed to initialize GTK".to_string());
        }

        // Create a GTK window — must be shown (realized) for WebKitGTK to composite.
        // Headless: use Popup type so it never appears in taskbar on any compositor.
        // Visible: use Toplevel for a normal debug window.
        let window_type = if visible {
            gtk::WindowType::Toplevel
        } else {
            gtk::WindowType::Popup
        };
        let window = gtk::Window::new(window_type);
        window.set_default_size(width as i32, height as i32);
        window.set_decorated(false);
        window.set_resizable(false);

        if !visible {
            window.set_opacity(0.0);
            window.set_skip_taskbar_hint(true);
            window.set_skip_pager_hint(true);
            window.set_accept_focus(false);
            window.move_(-10000, -10000);
        } else {
            window.set_title("encdr-view");
        }

        // Use a fixed container for precise sizing
        let fixed = gtk::Fixed::new();
        window.add(&fixed);

        // Show everything — this realizes the widget tree so WebKitGTK renders
        window.show_all();

        // Build the WebView inside the GTK container
        let frame_ready = Arc::new(Mutex::new(false));
        let frame_ready_ipc = frame_ready.clone();

        let webview = WebViewBuilder::new()
            .with_html(html)
            .with_initialization_script(bridge::BRIDGE_INIT_JS)
            .with_ipc_handler(move |request| {
                let body = request.body();
                if body == "__encdr_frame_ready" {
                    if let Ok(mut ready) = frame_ready_ipc.lock() {
                        *ready = true;
                    }
                }
            })
            .with_bounds(wry::Rect {
                position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, 0.0)),
                size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
                    width as f64,
                    height as f64,
                )),
            })
            .build_gtk(&fixed)
            .map_err(|e| format!("Failed to create WebView: {}", e))?;

        let webkit_view = webview.webview();

        // Pump GTK events briefly to let WebKit initialize
        for _ in 0..50 {
            gtk::main_iteration_do(false);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(Self {
            webview,
            webkit_view,
            _window: window,
            width,
            height,
            frame_ready,
        })
    }

    /// Check if the JS side has signaled that a new frame is ready.
    pub fn is_frame_ready(&self) -> bool {
        self.frame_ready
            .lock()
            .map(|ready| *ready)
            .unwrap_or(false)
    }

    /// Clear the frame-ready flag after capture.
    pub fn clear_frame_ready(&self) {
        if let Ok(mut ready) = self.frame_ready.lock() {
            *ready = false;
        }
    }

    /// Execute JavaScript in the WebView.
    pub fn eval(&self, js: &str) -> Result<(), String> {
        self.webview
            .evaluate_script(js)
            .map_err(|e| format!("JS eval failed: {}", e))
    }

    /// Load new HTML content.
    pub fn load_html(&self, html: &str) -> Result<(), String> {
        self.webview
            .load_html(html)
            .map_err(|e| format!("Failed to load HTML: {}", e))
    }
}

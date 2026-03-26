use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;

use crate::bridge;

// Thread-local event loop shared across all ManagedWebView instances.
thread_local! {
    static EVENT_LOOP: RefCell<Option<EventLoop<()>>> = RefCell::new(None);
}

fn with_event_loop<F, R>(f: F) -> R
where
    F: FnOnce(&mut EventLoop<()>) -> R,
{
    EVENT_LOOP.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(EventLoop::new());
        }
        f(opt.as_mut().unwrap())
    })
}

pub struct ManagedWebView {
    pub webview: wry::WebView,
    _window: tao::window::Window,
    pub width: u32,
    pub height: u32,
    pub frame_ready: Arc<Mutex<bool>>,
}

impl ManagedWebView {
    pub fn new(
        width: u32,
        height: u32,
        html: &str,
        visible: bool,
    ) -> Result<Self, String> {
        let frame_ready = Arc::new(Mutex::new(false));
        let frame_ready_ipc = frame_ready.clone();
        let html_owned = html.to_string();

        let mut window_out: Option<tao::window::Window> = None;
        let mut webview_out: Option<wry::WebView> = None;
        let mut error_out: Option<String> = None;

        with_event_loop(|event_loop| {
            event_loop.run_return(|event, target, control_flow| {
                if let Event::NewEvents(tao::event::StartCause::Init) = event {
                    let window = WindowBuilder::new()
                        .with_visible(visible)
                        .with_inner_size(tao::dpi::LogicalSize::new(width as f64, height as f64))
                        .with_decorations(visible)
                        .with_resizable(false)
                        .with_title(if visible { "encdr-view" } else { "" })
                        .build(target)
                        .map_err(|e| format!("Failed to create window: {}", e));

                    match window {
                        Ok(win) => {
                            let fr = frame_ready_ipc.clone();
                            let wv = wry::WebViewBuilder::new()
                                .with_html(&html_owned)
                                .with_initialization_script(bridge::BRIDGE_INIT_JS)
                                .with_ipc_handler(move |request| {
                                    if request.body() == "__encdr_frame_ready" {
                                        if let Ok(mut ready) = fr.lock() {
                                            *ready = true;
                                        }
                                    }
                                })
                                .with_bounds(wry::Rect {
                                    position: wry::dpi::Position::Logical(
                                        wry::dpi::LogicalPosition::new(0.0, 0.0),
                                    ),
                                    size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(
                                        width as f64,
                                        height as f64,
                                    )),
                                })
                                .build(&win);

                            match wv {
                                Ok(webview) => {
                                    window_out = Some(win);
                                    webview_out = Some(webview);
                                }
                                Err(e) => {
                                    error_out = Some(format!("Failed to create WebView: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            error_out = Some(e);
                        }
                    }

                    *control_flow = ControlFlow::Exit;
                }
            });
        });

        if let Some(e) = error_out {
            return Err(e);
        }

        let window = window_out.ok_or("Window was not created")?;
        let webview = webview_out.ok_or("WebView was not created")?;

        // Pump events briefly to let WebView2 initialize
        for _ in 0..50 {
            pump_events();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(Self {
            webview,
            _window: window,
            width,
            height,
            frame_ready,
        })
    }

    pub fn is_frame_ready(&self) -> bool {
        self.frame_ready.lock().map(|r| *r).unwrap_or(false)
    }

    pub fn clear_frame_ready(&self) {
        if let Ok(mut r) = self.frame_ready.lock() {
            *r = false;
        }
    }

    pub fn eval(&self, js: &str) -> Result<(), String> {
        self.webview
            .evaluate_script(js)
            .map_err(|e| format!("JS eval failed: {}", e))
    }

    pub fn load_html(&self, html: &str) -> Result<(), String> {
        self.webview
            .load_html(html)
            .map_err(|e| format!("Failed to load HTML: {}", e))
    }
}

/// Pump the platform event loop — processes pending Win32 messages and returns.
pub fn pump_events() {
    with_event_loop(|event_loop| {
        event_loop.run_return(|_event, _, control_flow| {
            *control_flow = ControlFlow::Exit;
        });
    });
}

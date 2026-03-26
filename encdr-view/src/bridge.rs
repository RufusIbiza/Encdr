use serde_json::Value;

/// Injected JavaScript that sets up the `window.encdr` bridge object.
///
/// App HTML calls `window.encdr.onMessage(channel, data)` — this is defined
/// by the app's JS code. We inject the plumbing that routes Rust→JS messages
/// to that handler, and JS→Rust signals back through `window.ipc.postMessage()`.
pub const BRIDGE_INIT_JS: &str = r#"
(function() {
    // Encdr bridge object — apps override `onMessage` to receive state updates
    if (!window.encdr) {
        window.encdr = {
            // Override this in your HTML to receive state pushes from Rust
            onMessage: function(channel, data) {},

            // Internal: signal Rust that the frame is dirty and ready for capture
            _dirty: false,
            _rafId: null,

            // Mark the view as needing a capture on next frame
            requestCapture: function() {
                if (!this._dirty) {
                    this._dirty = true;
                    if (!this._rafId) {
                        this._rafId = requestAnimationFrame(function() {
                            window.encdr._rafId = null;
                            if (window.encdr._dirty) {
                                window.encdr._dirty = false;
                                window.ipc.postMessage('__encdr_frame_ready');
                            }
                        });
                    }
                }
            }
        };
    }
})();
"#;

/// Build a JS snippet that calls `window.encdr.onMessage(channel, data)`
/// and then requests a capture.
pub fn build_send_js(channel: &str, data: &Value) -> String {
    let data_json = serde_json::to_string(data).unwrap_or_else(|_| "null".to_string());
    format!(
        r#"(function() {{
            if (window.encdr && window.encdr.onMessage) {{
                window.encdr.onMessage({ch}, {data});
            }}
            if (window.encdr && window.encdr.requestCapture) {{
                window.encdr.requestCapture();
            }}
        }})();"#,
        ch = serde_json::to_string(channel).unwrap_or_else(|_| "\"\"".to_string()),
        data = data_json,
    )
}

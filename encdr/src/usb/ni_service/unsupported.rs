use std::time::Duration;

/// The service route only exists on Windows.
pub struct ServiceSession(());

impl ServiceSession {
    pub fn connect(_product_id: u16, _serial: Option<&str>, _timeout: Duration) -> Result<Self, String> {
        Err("the NI service route is only available on Windows".to_string())
    }

    pub fn send_display(&self, _display: u8, _width: u16, _height: u16, _commands: &[u8]) -> Result<(), String> {
        Err("the NI service route is only available on Windows".to_string())
    }
}

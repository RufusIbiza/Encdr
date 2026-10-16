use std::time::Duration;

/// The service route only exists on Windows and macOS.
pub struct ServiceSession(());

impl ServiceSession {
    pub fn connect(_product_id: u16, _serial: Option<&str>, _timeout: Duration) -> Result<Self, String> {
        Err("the NI service route is only available on Windows and macOS".to_string())
    }

    pub fn send_display(&self, _display: u8, _width: u16, _height: u16, _commands: &[u8]) -> Result<(), String> {
        Err("the NI service route is only available on Windows and macOS".to_string())
    }
}

pub fn connect_with_retry(product_id: u16, serial: Option<&str>, timeout: Duration) -> Result<ServiceSession, String> {
    ServiceSession::connect(product_id, serial, timeout)
}

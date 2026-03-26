use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncdrError {
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    #[error("USB transfer error: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),

    #[error("Descriptor error: {0}")]
    Descriptor(String),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(std::io::Error),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device disconnected")]
    Disconnected,

    #[error("Screen not found: {0}")]
    ScreenNotFound(String),

    #[error("LED not found: {0}")]
    LedNotFound(String),

    #[error("GPU error: {0}")]
    Gpu(String),
}

pub type Result<T> = std::result::Result<T, EncdrError>;

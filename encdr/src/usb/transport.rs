use nusb::transfer::{RequestBuffer, TransferError};
use nusb::Interface;

/// Thin wrapper over nusb interface for USB read/write operations.
pub struct UsbTransport {
    interface: Interface,
}

impl UsbTransport {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    /// Read from an interrupt endpoint (blocking with timeout).
    pub async fn read_interrupt(
        &self,
        endpoint: u8,
        max_len: usize,
    ) -> Result<Vec<u8>, TransferError> {
        let buf = RequestBuffer::new(max_len);
        let result = self.interface.interrupt_in(endpoint, buf).await;
        result.into_result()
    }

    /// Write to an interrupt endpoint.
    pub async fn write_interrupt(
        &self,
        endpoint: u8,
        data: Vec<u8>,
    ) -> Result<(), TransferError> {
        self.interface.interrupt_out(endpoint, data).await.into_result()?;
        Ok(())
    }

    /// Write to a bulk endpoint (used for screen data).
    pub async fn write_bulk(
        &self,
        endpoint: u8,
        data: Vec<u8>,
    ) -> Result<(), TransferError> {
        self.interface.bulk_out(endpoint, data).await.into_result()?;
        Ok(())
    }

    /// Get a reference to the underlying nusb interface.
    pub fn inner(&self) -> &Interface {
        &self.interface
    }
}

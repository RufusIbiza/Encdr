use nusb::transfer::{Bulk, In, Interrupt, Out};
use nusb::Interface;

/// Thin wrapper over nusb interface for USB read/write operations.
pub struct UsbTransport {
    interface: Interface,
}

impl UsbTransport {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    /// Read from an interrupt endpoint.
    pub async fn read_interrupt(
        &self,
        endpoint: u8,
        max_len: usize,
    ) -> Result<Vec<u8>, String> {
        let mut ep = self
            .interface
            .endpoint::<Interrupt, In>(endpoint)
            .map_err(|e| e.to_string())?;
        let buf = ep.allocate(max_len);
        ep.submit(buf);
        let completion = ep.next_complete().await;
        completion.status.map_err(|e| e.to_string())?;
        Ok(completion.buffer[..completion.actual_len].to_vec())
    }

    /// Write to an interrupt endpoint.
    pub async fn write_interrupt(
        &self,
        endpoint: u8,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let mut ep = self
            .interface
            .endpoint::<Interrupt, Out>(endpoint)
            .map_err(|e| e.to_string())?;
        ep.submit(data.into());
        let completion = ep.next_complete().await;
        completion.status.map_err(|e| e.to_string())
    }

    /// Write to a bulk endpoint (used for screen data).
    pub async fn write_bulk(
        &self,
        endpoint: u8,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let mut ep = self
            .interface
            .endpoint::<Bulk, Out>(endpoint)
            .map_err(|e| e.to_string())?;
        ep.submit(data.into());
        let completion = ep.next_complete().await;
        completion.status.map_err(|e| e.to_string())
    }

    /// Get a reference to the underlying nusb interface.
    pub fn inner(&self) -> &Interface {
        &self.interface
    }
}

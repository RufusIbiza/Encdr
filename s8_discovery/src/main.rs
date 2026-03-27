use anyhow::{anyhow, Result};
use nusb::transfer::RequestBuffer;
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

use std::collections::HashMap;

struct S8Monitor {
    device: nusb::Device,
    interface: nusb::Interface,
    last_packets: HashMap<u8, Vec<u8>>,
}

impl S8Monitor {
    async fn new() -> Result<Self> {
        let mut devices = nusb::list_devices()?;
        let device_info = devices
            .find(|info: &nusb::DeviceInfo| {
                info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID
            })
            .ok_or_else(|| anyhow!("S8 device not found (VID:PID {:04x}:{:04x})", S8_VENDOR_ID, S8_PRODUCT_ID))?;

        println!("Found: {} by {}",
                 device_info.product_string().unwrap_or("S8"),
                 device_info.manufacturer_string().unwrap_or("NI"));
        println!("  Bus: {}, Device: {}", device_info.bus_number(), device_info.device_address());

        let device = device_info.open()?;

        let interface = device.detach_and_claim_interface(5)?;
        println!("Claimed interface 5 (HID control)");

        Ok(S8Monitor {
            device,
            interface,
            last_packets: HashMap::new(),
        })
    }

    async fn read_control_packet(interface: &nusb::Interface) -> Result<Vec<u8>> {
        let buf = RequestBuffer::new(256);
        match tokio::time::timeout(
            Duration::from_millis(100),
            interface.interrupt_in(0x84, buf)
        ).await {
            Ok(completion) => {
                match completion.into_result() {
                    Ok(data) => Ok(data),
                    Err(_e) => {
                        Ok(vec![])
                    }
                }
            }
            Err(_) => {
                Ok(vec![])
            }
        }
    }

    async fn run_monitor(&mut self) -> Result<()> {
        println!("\n╔════════════════════════════════════════════════════════════╗");
        println!("║         S8 USB HID Control Monitor (Multi-Report)          ║");
        println!("║                                                            ║");
        println!("║  Manipulate controls on the S8. Changes will be printed.   ║");
        println!("╚════════════════════════════════════════════════════════════╝\n");

        loop {
            if let Ok(packet) = Self::read_control_packet(&self.interface).await {
                if !packet.is_empty() {
                    let id = packet[0];
                    if let Some(last) = self.last_packets.get(&id) {
                        if &packet != last {
                            self.print_packet_diff(id, last, &packet);
                        }
                    } else {
                        println!("Received initial packet for Report ID 0x{:02x} ({} bytes).", id, packet.len());
                    }
                    self.last_packets.insert(id, packet);
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn print_packet_diff(&self, id: u8, old: &[u8], new: &[u8]) {
        let max_len = std::cmp::max(old.len(), new.len());
        let mut diff_output = String::new();
        for i in 0..max_len {
            let old_byte = old.get(i).copied().unwrap_or(0);
            let new_byte = new.get(i).copied().unwrap_or(0);
            if old_byte != new_byte {
                let diff_bits = old_byte ^ new_byte;
                diff_output.push_str(&format!(
                    "  Byte[{:03}]: 0x{:02x} -> 0x{:02x} | 0b{:08b} -> 0b{:08b} (diff mask: 0x{:02x})\n",
                    i, old_byte, new_byte, old_byte, new_byte, diff_bits
                ));
            }
        }
        
        if !diff_output.is_empty() {
            println!("Packet change detected for Report ID 0x{:02x}:", id);
            print!("{}", diff_output);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?),
        )
        .init();

    let mut monitor = S8Monitor::new().await?;
    monitor.run_monitor().await?;
    Ok(())
}

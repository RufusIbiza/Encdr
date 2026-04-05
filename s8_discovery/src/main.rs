use anyhow::{anyhow, Result};
use nusb::transfer::RequestBuffer;
use std::time::{Duration, Instant};
use std::fs::OpenOptions;
use std::io::Write;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

use std::collections::HashMap;

struct S8Monitor {
    device: nusb::Device,
    interface: nusb::Interface,
    last_packets: HashMap<u8, Vec<u8>>,
    start_time: Instant,
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
            start_time: Instant::now(),
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
        println!("║  Captures ALL report types (41B, 109B, etc)                ║");
        println!("║  Position data is in Sliders report (109B), bytes [2,3]   ║");
        println!("║  Logging all packets to: /tmp/s8_hid_packets.log          ║");
        println!("╚════════════════════════════════════════════════════════════╝\n");
        println!("Starting packet capture. Touch ONLY the left touchstrip.\n");

        // Create CSV header with 128 bytes (covers all known report types)
        let mut header = "timestamp_ms,packet_num,report_id,packet_size".to_string();
        for i in 0..128 {
            header.push_str(&format!(",byte_{}", i));
        }
        header.push('\n');
        let _ = std::fs::write("/tmp/s8_hid_packets.log", header);

        let mut packet_count = 0;
        let mut report_types: HashMap<u8, usize> = HashMap::new();

        loop {
            if let Ok(packet) = Self::read_control_packet(&self.interface).await {
                if !packet.is_empty() {
                    packet_count += 1;
                    let id = packet[0];
                    let size = packet.len();

                    // Track report types seen
                    *report_types.entry(id).or_insert(0) += 1;

                    // Log every packet to file
                    self.log_packet_to_file(packet_count, id, &packet);

                    if let Some(last) = self.last_packets.get(&id) {
                        if &packet != last {
                            self.print_packet_diff(id, last, &packet);
                        }
                    } else {
                        let type_name = match size {
                            41 => "Buttons Report",
                            109 => "Sliders Report (POSITION DATA!)",
                            _ => "Unknown Report",
                        };
                        println!("✓ Report ID 0x{:02x}: {} ({} bytes) - Contains: {}", id, type_name, size, self.identify_report_content(size));
                    }
                    self.last_packets.insert(id, packet);
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn identify_report_content(&self, size: usize) -> &'static str {
        match size {
            41 => "buttons, encoders, xfader",
            109 => "sliders, faders, touchstrip POSITION [2,3] and [34,35]",
            _ => "unknown",
        }
    }

    fn log_packet_to_file(&self, packet_num: usize, id: u8, packet: &[u8]) {
        let elapsed_ms = self.start_time.elapsed().as_millis();

        // Build CSV line: timestamp, packet_num, report_id, packet_size, then all bytes as decimals
        let mut csv_line = format!("{},{},{},{}", elapsed_ms, packet_num, id, packet.len());

        // Add up to 128 bytes (covers all known reports)
        for i in 0..128 {
            let byte_val = packet.get(i).copied().unwrap_or(0);
            csv_line.push(',');
            csv_line.push_str(&byte_val.to_string());
        }
        csv_line.push('\n');

        if let Ok(mut file) = OpenOptions::new()
            .append(true)
            .open("/tmp/s8_hid_packets.log")
        {
            let _ = file.write_all(csv_line.as_bytes());
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

            // Add touchstrip interpretation for buttons packet
            if id == 0x01 && new.len() >= 35 {
                let left_26_27 = (new[26] as u16) | ((new[27] as u16) << 8);
                let left_32_33 = (new[32] as u16) | ((new[33] as u16) << 8);
                let right_34_35 = (new[34] as u16) | ((new[35] as u16) << 8);
                let byte_28 = new[28];
                let byte_29 = new[29];

                println!("  [TOUCHSTRIP DATA]");
                println!("    Left[26,27]:   0x{:04x}  |  Left[32,33]:   0x{:04x}", left_26_27, left_32_33);
                println!("    Right[34,35]:  0x{:04x}  |  Byte[28]: 0x{:02x}  Byte[29]: 0x{:02x}", right_34_35, byte_28, byte_29);

                // Print full hex dump
                self.print_full_hex_dump(new);

                // Detect which touchstrip is active
                let old_byte_29 = old.get(29).copied().unwrap_or(0);
                if old_byte_29 != byte_29 {
                    if old_byte_29 == 0x00 && byte_29 != 0x00 {
                        println!("    ⭐ LEFT TOUCHSTRIP TOUCH DETECTED (byte[29]: 0x00 → 0x{:02x})", byte_29);
                    } else if old_byte_29 != 0x00 && byte_29 == 0x00 {
                        println!("    ⭐ LEFT TOUCHSTRIP RELEASED (byte[29]: 0x{:02x} → 0x00)", old_byte_29);
                    } else {
                        println!("    ℹ  Byte[29] cycling: 0x{:02x} → 0x{:02x} (left strip active)", old_byte_29, byte_29);
                    }
                }
                println!();
            }
        }
    }

    fn print_full_hex_dump(&self, packet: &[u8]) {
        println!("  [FULL HEX DUMP]");
        for (i, chunk) in packet.chunks(16).enumerate() {
            print!("    [{:02}] ", i * 16);
            for byte in chunk {
                print!("{:02x} ", byte);
            }
            println!();
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

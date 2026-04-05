//! Detailed S8 HID packet analyzer.
//!
//! Run with: cargo run -p encdr --example s8_discovery_detailed
//!
//! Shows full packet dumps with byte-by-byte highlighting.

use nusb::list::Filter;
use std::time::Duration;

fn main() {
    println!("=== S8 HID Packet Analyzer (Detailed) ===\n");
    println!("Scanning for NI Kontrol S8...");

    // Find S8 device
    let devices = nusb::list::buses()
        .filter_map(|bus| {
            bus.devices()
                .find(|dev| {
                    if let Ok(desc) = dev.device_descriptor() {
                        desc.vendor_id == 0x17cc && desc.product_id == 0x1370
                    } else {
                        false
                    }
                })
        })
        .collect::<Vec<_>>();

    if devices.is_empty() {
        eprintln!("S8 device not found. Check USB connection.");
        return;
    }

    let device = &devices[0];
    let mut handle = match device.open() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to open device: {}", e);
            return;
        }
    };

    // Claim interface 5 (control)
    if let Err(e) = handle.claim_interface(5) {
        eprintln!("Failed to claim interface: {}", e);
        return;
    }

    println!("Connected! Press Ctrl+C to stop.\n");
    println!("Move/touch touchstrips to see detailed packet changes.\n");
    println!("{}", "=".repeat(100));

    let mut last_packet: Vec<u8> = vec![0u8; 41];
    let mut packet_count = 0u64;

    loop {
        let mut packet = vec![0u8; 41];
        match handle.read_interrupt(0x84, &mut packet, Duration::from_secs(1)) {
            Ok(_) => {
                if packet == last_packet {
                    continue; // Skip unchanged packets
                }

                packet_count += 1;

                // Find which bytes changed
                let mut changed_ranges: Vec<(usize, usize)> = Vec::new();
                let mut in_range = false;
                let mut range_start = 0;

                for i in 0..packet.len() {
                    if packet[i] != last_packet[i] {
                        if !in_range {
                            range_start = i;
                            in_range = true;
                        }
                    } else if in_range {
                        changed_ranges.push((range_start, i - 1));
                        in_range = false;
                    }
                }
                if in_range {
                    changed_ranges.push((range_start, packet.len() - 1));
                }

                // Show packet header
                println!("\n[Packet {}]", packet_count);

                // Print full hex dump with highlighting
                for i in 0..packet.len() {
                    let is_changed = changed_ranges.iter().any(|(s, e)| i >= *s && i <= *e);

                    if i % 16 == 0 {
                        print!("\n  [{}] ", i);
                    }

                    if is_changed {
                        print!("\x1b[1;33m{:02x}\x1b[0m ", packet[i]); // Yellow for changed
                    } else {
                        print!("{:02x} ", packet[i]);
                    }
                }
                println!("\n");

                // Show changed ranges with details
                if !changed_ranges.is_empty() {
                    println!("  Changed bytes:");
                    for (start, end) in changed_ranges {
                        for i in start..=end {
                            println!(
                                "    Byte[{}]: 0x{:02x} -> 0x{:02x} | 0b{:08b} -> 0b{:08b}",
                                i, last_packet[i], packet[i], last_packet[i], packet[i]
                            );
                        }
                    }
                }

                // Highlight interesting byte groups
                let left_pos = (packet[26] as u16) | ((packet[27] as u16) << 8);
                let left_pos_alt = (packet[32] as u16) | ((packet[33] as u16) << 8);
                let right_pos = (packet[34] as u16) | ((packet[35] as u16) << 8);

                println!(
                    "  [KEY BYTES] Left[26,27]: 0x{:04x}  Left[32,33]: 0x{:04x}  Right[34,35]: 0x{:04x}  [028]: 0x{:02x}  [029]: 0x{:02x}",
                    left_pos, left_pos_alt, right_pos, packet[28], packet[29]
                );

                last_packet = packet;
            }
            Err(_) => continue,
        }
    }
}

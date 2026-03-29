use anyhow::{anyhow, Result};
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        "set" => {
            if args.len() < 5 {
                println!("Usage: led_test set <prefix_hex> <offset> <value>");
                return Ok(());
            }
            let prefix = parse_hex(&args[2])?;
            let offset: usize = args[3].parse()?;
            let value: u8 = args[4].parse()?;
            send_led_report(prefix, vec![(offset, value)]).await?;
        }
        "sweep" => {
            if args.len() < 3 {
                println!("Usage: led_test sweep <prefix_hex> [start_offset]");
                return Ok(());
            }
            let prefix = parse_hex(&args[2])?;
            let start: usize = args.get(3).map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
            
            for offset in start..200 {
                println!("Testing offset {} with prefix 0x{:02x}...", offset, prefix);
                send_led_report(prefix, vec![(offset, 127)]).await?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                // Turn it off before next
                send_led_report(prefix, vec![(offset, 0)]).await?;
            }
        }
        "all_off" => {
            for &prefix in &[0x80u8, 0x81, 0x82] {
                println!("Turning off all LEDs for prefix 0x{:02x}...", prefix);
                let buffer = vec![0u8; 308]; // Data only
                send_raw_report(prefix, &buffer).await?;
            }
        }
        "all_on" => {
            if args.len() < 3 {
                println!("Usage: led_test all_on <prefix_hex>");
                return Ok(());
            }
            let prefix = parse_hex(&args[2])?;
            println!("Turning on all LEDs for prefix 0x{:02x}...", prefix);
            let buffer = vec![127u8; 308]; // Data only
            send_raw_report(prefix, &buffer).await?;
        }
        "brute_prefix" => {
            let offset: usize = args.get(2).map(|s| s.parse().unwrap_or(25)).unwrap_or(25);
            for p in 0x00u8..=0xff {
                println!("Testing prefix 0x{:02x} (offset {})...", p, offset);
                let _ = send_raw_report(p, &[127u8; 308]).await;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        "wakeup" => {
            println!("Sending wakeup command (0x01)...");
            // Try sending a single 0x01 byte to Interface 5, EP 0x03
            send_raw_report_custom_size(0x01, &[], 1).await?;
        }
        _ => print_usage(),
    }

    Ok(())
}

async fn send_raw_report_custom_size(prefix: u8, data: &[u8], total_size: usize) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; total_size];
    buffer[0] = prefix;
    if total_size > 1 {
        let copy_len = std::cmp::min(data.len(), total_size - 1);
        buffer[1..1+copy_len].copy_from_slice(&data[..copy_len]);
    }

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}

fn print_usage() {
    println!("S8 LED Test Tool");
    println!("Usage:");
    println!("  led_test set <prefix_hex> <offset> <value>  - Set a single LED");
    println!("  led_test sweep <prefix_hex> [start]         - Sweep through offsets");
    println!("  led_test all_off                            - Turn off all LEDs");
    println!("  led_test all_on <prefix_hex>               - Turn on all LEDs for a prefix");
    println!("  led_test brute_prefix [offset]              - Brute-force through Report IDs");
    println!("  led_test wakeup                             - Send device wakeup (HID enable)");
    println!("\nExamples:");
    println!("  led_test set 0x80 59 127   (Left Play ON)");
    println!("  led_test set 0x81 59 127   (Right Play ON)");
}

fn parse_hex(s: &str) -> Result<u8> {
    u8::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| anyhow!("Invalid hex: {}", e))
}

async fn send_led_report(prefix: u8, updates: Vec<(usize, u8)>) -> Result<()> {
    let mut data = vec![0u8; 308];
    for (offset, value) in updates {
        if offset < data.len() {
            data[offset] = value;
        }
    }
    send_raw_report(prefix, &data).await
}

async fn send_raw_report(prefix: u8, data: &[u8]) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;

    // We assume interface 5 is the HID one as in main.rs
    let interface = device.detach_and_claim_interface(5)?;

    // Try with Report ID 0x80 (the LED prefix itself) as the Report ID
    let mut buffer = vec![0u8; 310];
    buffer[0] = prefix;  // This will be the Report ID in HID terms
    buffer[1] = prefix;  // Also set the prefix in the data
    let copy_len = std::cmp::min(data.len(), 308);
    buffer[2..2+copy_len].copy_from_slice(&data[..copy_len]);

    println!("Sending LED report with Report ID 0x{:02x}, prefix 0x{:02x}", prefix, prefix);

    // Interface 5 has OUT endpoint 0x03
    match interface.interrupt_out(0x03, buffer).await.into_result() {
        Ok(_) => {
            println!("  ✓ Sent successfully");
            Ok(())
        },
        Err(e) => {
             println!("  ✗ Endpoint 0x03 failed: {}.", e);
             Err(anyhow!("Failed to send to 0x03: {}", e))
        }
    }
}

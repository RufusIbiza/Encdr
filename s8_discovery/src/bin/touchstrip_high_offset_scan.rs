/// Scan offsets 122–308 and alternate prefixes (0x83–0x8f) for touchstrip LEDs.
///
/// Usage:
///   cargo run --bin touchstrip_high_offset_scan -- high    (scan offsets 122-308, prefix 0x80)
///   cargo run --bin touchstrip_high_offset_scan -- prefix  (try prefixes 0x83-0x8f at D2 offsets 68-117)
///   cargo run --bin touchstrip_high_offset_scan -- both    (try prefixes 0x83-0x8f at offsets 122-308)

use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

fn open_iface() -> Result<nusb::Interface> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;
    let device = device_info.open()?;
    Ok(device.detach_and_claim_interface(5)?)
}

async fn send(iface: &nusb::Interface, prefix: u8, offsets: &[usize]) -> Result<()> {
    let mut buf = vec![0u8; 309];
    buf[0] = prefix;
    for &o in offsets {
        if o < 308 {
            buf[1 + o] = 255;
        }
    }
    iface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

async fn clear(iface: &nusb::Interface, prefix: u8) -> Result<()> {
    let mut buf = vec![0u8; 309];
    buf[0] = prefix;
    iface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

/// Send LEDs first, then ask. Returns true if user saw something.
async fn test_range(iface: &nusb::Interface, prefix: u8, start: usize, end: usize, label: &str) -> Result<bool> {
    let offsets: Vec<usize> = (start..=end).collect();
    send(iface, prefix, &offsets).await?;
    print!("  {} (prefix=0x{:02x}, offsets {}-{}): touchstrip lit? [ENTER=no, type anything=YES]: ",
        label, prefix, start, end);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    clear(iface, prefix).await?;
    Ok(!input.trim().is_empty())
}

async fn test_single(iface: &nusb::Interface, prefix: u8, offset: usize) -> Result<bool> {
    send(iface, prefix, &[offset]).await?;
    print!("  offset {:3} (0x{:02x}): [ENTER=nothing, type anything=FOUND]: ", offset, offset);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    clear(iface, prefix).await?;
    Ok(!input.trim().is_empty())
}

/// Binary search then individual scan of a range.
async fn bisect(iface: &nusb::Interface, prefix: u8, start: usize, end: usize) -> Result<()> {
    if start == end {
        if test_single(iface, prefix, start).await? {
            println!("\n*** FOUND: prefix=0x{:02x} offset={} ***\n", prefix, start);
        }
        return Ok(());
    }
    if end - start <= 8 {
        // Small enough to scan individually
        for offset in start..=end {
            if test_single(iface, prefix, offset).await? {
                println!("\n*** FOUND: prefix=0x{:02x} offset={} ***\n", prefix, offset);
            }
        }
        return Ok(());
    }
    let mid = (start + end) / 2;
    if test_range(iface, prefix, start, mid, "lower half").await? {
        Box::pin(bisect(iface, prefix, start, mid)).await?;
    }
    if test_range(iface, prefix, mid + 1, end, "upper half").await? {
        Box::pin(bisect(iface, prefix, mid + 1, end)).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "high".to_string());

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Touchstrip High-Offset / Alt-Prefix Scanner              ║");
    println!("║   LEDs are sent THEN you report what you see               ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let iface = open_iface()?;

    match mode.as_str() {
        "high" => {
            println!("Scanning offsets 122–308 with prefix 0x80\n");
            if test_range(&iface, 0x80, 122, 308, "offsets 122-308").await? {
                println!("Something responded! Binary searching...\n");
                bisect(&iface, 0x80, 122, 308).await?;
            } else {
                println!("No response in offsets 122-308 with prefix 0x80.");
                println!("Try: cargo run --bin touchstrip_high_offset_scan -- prefix");
            }
        }
        "prefix" => {
            println!("Trying alternate prefixes 0x83–0x8f at D2 touchstrip offsets (68–117)\n");
            for prefix in 0x83u8..=0x8f {
                if test_range(&iface, prefix, 68, 117, "D2 touchstrip offsets").await? {
                    println!("Response on prefix 0x{:02x}! Bisecting...\n", prefix);
                    bisect(&iface, prefix, 68, 117).await?;
                } else {
                    println!("  No response on prefix 0x{:02x}.", prefix);
                }
            }
        }
        "both" => {
            println!("Trying alternate prefixes 0x83–0x8f at high offsets (122–250)\n");
            for prefix in 0x83u8..=0x8f {
                if test_range(&iface, prefix, 122, 250, "high offsets").await? {
                    println!("Response on prefix 0x{:02x}! Bisecting...\n", prefix);
                    bisect(&iface, prefix, 122, 250).await?;
                } else {
                    println!("  No response on prefix 0x{:02x}.", prefix);
                }
            }
        }
        _ => {
            eprintln!("Unknown mode '{}'. Use: high | prefix | both", mode);
        }
    }

    // Final clear of all known prefixes
    for prefix in [0x80u8, 0x81, 0x82, 0x83, 0x84, 0x85] {
        clear(&iface, prefix).await.ok();
    }
    println!("\nDone — all LEDs cleared.");
    Ok(())
}

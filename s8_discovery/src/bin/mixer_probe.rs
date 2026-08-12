/// Interactive mixer LED probe for NI Kontrol S8.
///
/// Usage:
///   mixer_probe <prefix_hex> [start_offset] [output_file]
///
/// Example:
///   mixer_probe 0x82 0 probe_0x82.json
///
/// For each offset 0..308:
///   - Blinks that LED on (127) then off (0)
///   - Waits for you to press Enter (nothing lit) or type a description
/// Saves results to JSON when done or interrupted.

use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mixer_probe <prefix_hex> [start_offset] [output_file]");
        eprintln!("Example: mixer_probe 0x82 0 probe_0x82.json");
        std::process::exit(1);
    }

    let prefix = parse_hex(&args[1])?;
    let start: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let output_file = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| format!("probe_0x{:02x}.json", prefix));

    println!("=== S8 Mixer LED Probe ===");
    println!("Prefix: 0x{:02x}  Start offset: {}  Output: {}", prefix, start, output_file);
    println!();

    // Send handshake: exactly 2 bytes [0xf3, 0x01] as a separate interrupt transfer.
    println!("Sending handshake [0xf3, 0x01]...");
    send_handshake().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    println!("Handshake sent. Mixer LEDs should all be on.");
    println!();
    println!("For each offset I will blink the LED on then off.");
    println!("Press Enter if nothing visible, or type what you see (e.g. 'cue_a') then Enter.");
    println!("Type 'q' to quit and save, 'r' to resend handshake.");
    println!("-------------------------------------------------------");

    let mut results: BTreeMap<usize, String> = BTreeMap::new();

    'outer: for offset in start..309 {
        // Spawn a background task that blinks the LED until we set stop=true.
        let stop = Arc::new(AtomicBool::new(false));
        let stop_blink = Arc::clone(&stop);
        let blink_task = tokio::spawn(async move {
            while !stop_blink.load(Ordering::Relaxed) {
                let on = make_buf(offset, 127);
                let _ = send_packet(prefix, &on).await;
                tokio::time::sleep(Duration::from_millis(300)).await;
                if stop_blink.load(Ordering::Relaxed) { break; }
                let off = make_buf(offset, 0);
                let _ = send_packet(prefix, &off).await;
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            // Leave the LED off when done.
            let off = make_buf(offset, 0);
            let _ = send_packet(prefix, &off).await;
        });

        print!("  offset {:3} → ", offset);
        io::stdout().flush()?;

        // Read input on the blocking thread pool so the blink task keeps running.
        let line = tokio::task::spawn_blocking(|| {
            let mut s = String::new();
            io::stdin().read_line(&mut s).ok();
            s
        }).await?;

        // Stop blinking.
        stop.store(true, Ordering::Relaxed);
        blink_task.await?;

        let trimmed = line.trim().to_string();

        if trimmed == "q" {
            println!("Quitting at offset {}.", offset);
            break 'outer;
        }
        if trimmed == "r" {
            println!("  Resending handshake...");
            send_handshake().await?;
            tokio::time::sleep(Duration::from_millis(200)).await;
            // Redo this offset with blinking again.
            let stop2 = Arc::new(AtomicBool::new(false));
            let stop_blink2 = Arc::clone(&stop2);
            let blink_task2 = tokio::spawn(async move {
                while !stop_blink2.load(Ordering::Relaxed) {
                    let on = make_buf(offset, 127);
                    let _ = send_packet(prefix, &on).await;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    if stop_blink2.load(Ordering::Relaxed) { break; }
                    let off = make_buf(offset, 0);
                    let _ = send_packet(prefix, &off).await;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                let off = make_buf(offset, 0);
                let _ = send_packet(prefix, &off).await;
            });
            print!("  offset {:3} (retry) → ", offset);
            io::stdout().flush()?;
            let line2 = tokio::task::spawn_blocking(|| {
                let mut s = String::new();
                io::stdin().read_line(&mut s).ok();
                s
            }).await?;
            stop2.store(true, Ordering::Relaxed);
            blink_task2.await?;
            let trimmed2 = line2.trim().to_string();
            if !trimmed2.is_empty() && trimmed2 != "q" {
                results.insert(offset, trimmed2.clone());
            }
            if trimmed2 == "q" {
                break 'outer;
            }
        } else if !trimmed.is_empty() {
            results.insert(offset, trimmed);
        }
    }

    // Save results
    save_results(prefix, &results, &output_file)?;
    println!();
    println!("Saved to {}", output_file);
    if results.is_empty() {
        println!("No observations recorded.");
    } else {
        println!("Recorded {} observations:", results.len());
        for (offset, label) in &results {
            println!("  0x{:02x} offset {:3} (abs LED {:3}): {}", prefix, offset, offset, label);
        }
    }

    Ok(())
}

fn make_buf(offset: usize, value: u8) -> Vec<u8> {
    let mut data = vec![0u8; 308];
    if offset < 308 {
        data[offset] = value;
    }
    data
}

async fn send_handshake() -> Result<()> {
    let device_info = find_s8()?;
    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;
    // 2-byte handshake: [0xf3, 0x01]
    let buf = vec![0xf3u8, 0x01];
    interface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

async fn send_packet(prefix: u8, data: &[u8]) -> Result<()> {
    let device_info = find_s8()?;
    let device = device_info.open()?;
    let interface = device.detach_and_claim_interface(5)?;

    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;
    let copy_len = data.len().min(308);
    buffer[1..1 + copy_len].copy_from_slice(&data[..copy_len]);

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}

fn find_s8() -> Result<nusb::DeviceInfo> {
    nusb::list_devices()?
        .find(|d| d.vendor_id() == S8_VENDOR_ID && d.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))
}

fn save_results(prefix: u8, results: &BTreeMap<usize, String>, path: &str) -> Result<()> {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"prefix\": \"0x{:02x}\",\n", prefix));
    out.push_str("  \"observations\": {\n");
    let entries: Vec<_> = results.iter().collect();
    for (i, (offset, label)) in entries.iter().enumerate() {
        let comma = if i + 1 < entries.len() { "," } else { "" };
        out.push_str(&format!(
            "    \"{}\": \"{}\"{}  /* abs LED {} */\n",
            offset, label, comma, offset
        ));
    }
    out.push_str("  }\n}\n");
    std::fs::write(path, out)?;
    Ok(())
}

fn parse_hex(s: &str) -> Result<u8> {
    u8::from_str_radix(s.trim_start_matches("0x"), 16)
        .map_err(|e| anyhow!("Invalid hex '{}': {}", s, e))
}

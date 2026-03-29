/// Map touchstrip LED colours now that the segment format is confirmed:
/// - Each deck report is 119 bytes: [prefix] + [118 data bytes]
/// - Touchstrip starts at relative offset 93 within the segment
/// - Each of the 25 LEDs uses 2 bytes (one colour per byte)
///   offset 93 = LED 0 colour A, offset 94 = LED 0 colour B,
///   offset 95 = LED 1 colour A, offset 96 = LED 1 colour B, etc.
///
/// Tests:
///   A — confirm left strip: prefix 0x80, 119-byte report, offsets 93-117
///   B — colour mapping: light even bytes (93,95,...) vs odd bytes (94,96,...)
///   C — confirm right strip with same layout
///   D — individual LED walk to confirm per-LED addressing
///
/// Run: cargo run --bin touchstrip_colour_map

use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

fn prompt(label: &str) -> Result<bool> {
    print!("  {} [ENTER=nothing, type colour/description=FOUND]: ", label);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_string();
    if !trimmed.is_empty() {
        println!("  → {}", trimmed);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Send a 119-byte segmented report.
async fn send_seg(iface: &nusb::Interface, prefix: u8, offsets: &[usize]) -> Result<()> {
    let mut buf = vec![0u8; 119];
    buf[0] = prefix;
    for &o in offsets {
        if 1 + o < buf.len() {
            buf[1 + o] = 255;
        }
    }
    iface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

async fn clear_seg(iface: &nusb::Interface, prefix: u8) -> Result<()> {
    let buf = vec![0u8; 119];
    let mut buf = buf;
    buf[0] = prefix;
    iface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;
    let device = device_info.open()?;
    let iface = device.detach_and_claim_interface(5)?;

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Touchstrip Colour Mapping                                ║");
    println!("║   119-byte segmented report: [prefix] + [118 bytes]       ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // ── A: Confirm left strip with 0x80 short report ─────────────────────
    println!("═══ A: Left strip — 0x80, 119 bytes, relative offsets 93-117 ═══\n");
    send_seg(&iface, 0x80, &(93..=142).collect::<Vec<_>>()).await?;
    prompt("A1: Left touchstrip lit? (all 50 bytes 93-142)")?;
    clear_seg(&iface, 0x80).await?;

    send_seg(&iface, 0x80, &(93..=117).collect::<Vec<_>>()).await?;
    prompt("A2: Left touchstrip — first 25 bytes only (93-117)?")?;
    clear_seg(&iface, 0x80).await?;

    // ── B: Colour mapping — even vs odd byte positions ────────────────────
    println!("\n═══ B: Colour mapping ═══\n");
    println!("  Each LED = 2 bytes. Testing even-index (93,95,97...) vs odd-index (94,96,98...).\n");

    // Even offsets: 93, 95, 97 ... 141 (25 LEDs, first byte of each pair)
    let evens: Vec<usize> = (0..25).map(|i| 93 + i * 2).collect();
    // Odd offsets: 94, 96, 98 ... 142 (25 LEDs, second byte of each pair)
    let odds: Vec<usize> = (0..25).map(|i| 94 + i * 2).collect();

    send_seg(&iface, 0x80, &evens).await?;
    prompt("B1: LEFT — even bytes (93,95,...141) — what colour?")?;
    clear_seg(&iface, 0x80).await?;

    send_seg(&iface, 0x80, &odds).await?;
    prompt("B2: LEFT — odd bytes (94,96,...142) — what colour?")?;
    clear_seg(&iface, 0x80).await?;

    // Same for right
    send_seg(&iface, 0x81, &evens).await?;
    prompt("B3: RIGHT — even bytes (93,95,...141) — what colour?")?;
    clear_seg(&iface, 0x81).await?;

    send_seg(&iface, 0x81, &odds).await?;
    prompt("B4: RIGHT — odd bytes (94,96,...142) — what colour?")?;
    clear_seg(&iface, 0x81).await?;

    // ── C: Gradient — light LEDs one at a time from left to right ─────────
    println!("\n═══ C: Individual LED walk — left strip (press ENTER for each) ═══\n");
    println!("  This confirms each LED is independently addressable.\n");
    for i in 0..25usize {
        let even = 93 + i * 2;
        let odd  = 94 + i * 2;
        // Light both bytes for this LED (both colours)
        send_seg(&iface, 0x80, &[even, odd]).await?;
        print!("  LED {:2} (bytes {} + {}): [ENTER to advance]: ", i + 1, even, odd);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() {
            println!("  → note: {}", input.trim());
        }
        clear_seg(&iface, 0x80).await?;
    }

    // ── D: Brightness levels ──────────────────────────────────────────────
    println!("\n═══ D: Brightness levels — are values 0-255 or 0/1? ═══\n");
    for &brightness in &[32u8, 64, 128, 255] {
        let mut buf = vec![0u8; 119];
        buf[0] = 0x80;
        for i in 0..25usize {
            buf[1 + 93 + i * 2] = brightness; // even bytes (one colour)
        }
        iface.interrupt_out(0x03, buf).await.into_result()?;
        prompt(&format!("D: Left strip at brightness {} — brighter than previous?", brightness))?;
    }
    clear_seg(&iface, 0x80).await?;

    println!("\nDone — all LEDs cleared.");
    Ok(())
}

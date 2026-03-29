/// Test the blue touchstrip channel with the confirmed 119-byte segmented format.
///
/// We now know:
///   - 119-byte report: [prefix] + [118 data bytes]
///   - Orange: relative offsets 93-117 (25 bytes, 1 per LED)
///   - Blue: hypothesis = relative offsets 68-92 (25 bytes, matching D2 layout)
///
/// Run: cargo run --bin touchstrip_blue_test

use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

fn prompt(label: &str) -> Result<String> {
    print!("  {} [ENTER=nothing, describe what you see]: ", label);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_string();
    if !trimmed.is_empty() {
        println!("  → {}", trimmed);
    }
    Ok(trimmed)
}

async fn send_seg(iface: &nusb::Interface, prefix: u8, offsets: &[usize], brightness: u8) -> Result<()> {
    let mut buf = vec![0u8; 119];
    buf[0] = prefix;
    for &o in offsets {
        if 1 + o < buf.len() {
            buf[1 + o] = brightness;
        }
    }
    iface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

async fn clear(iface: &nusb::Interface, prefix: u8) -> Result<()> {
    let mut buf = vec![0u8; 119];
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
    println!("║   Touchstrip Blue Channel Test                             ║");
    println!("║   119-byte segmented report, prefix 0x80 (left)           ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let blue_offsets: Vec<usize>   = (68..=92).collect();
    let orange_offsets: Vec<usize> = (93..=117).collect();

    // ── A: Blue only ──────────────────────────────────────────────────────
    println!("═══ A: Blue channel only (offsets 68-92) ═══\n");
    send_seg(&iface, 0x80, &blue_offsets, 255).await?;
    prompt("A1: Left — blue LEDs lit? What colour?")?;
    clear(&iface, 0x80).await?;

    send_seg(&iface, 0x81, &blue_offsets, 255).await?;
    prompt("A2: Right — blue LEDs lit? What colour?")?;
    clear(&iface, 0x81).await?;

    // ── B: Orange only (confirmed working, sanity check) ─────────────────
    println!("\n═══ B: Orange only (offsets 93-117) — sanity check ═══\n");
    send_seg(&iface, 0x80, &orange_offsets, 255).await?;
    prompt("B1: Left orange (should be all orange)?")?;
    clear(&iface, 0x80).await?;

    // ── C: Both simultaneously ────────────────────────────────────────────
    println!("\n═══ C: Both blue (68-92) + orange (93-117) simultaneously ═══\n");
    let both: Vec<usize> = (68..=117).collect();
    send_seg(&iface, 0x80, &both, 255).await?;
    prompt("C1: Left — both colours lit? Do blue and orange LEDs show?")?;
    clear(&iface, 0x80).await?;

    send_seg(&iface, 0x81, &both, 255).await?;
    prompt("C2: Right — both colours lit?")?;
    clear(&iface, 0x81).await?;

    // ── D: Individual blue LED walk ───────────────────────────────────────
    println!("\n═══ D: Blue LED walk — left strip (press ENTER for each) ═══\n");
    for i in 0..25usize {
        let offset = 68 + i;
        send_seg(&iface, 0x80, &[offset], 255).await?;
        print!("  LED {:2} (offset {:3}): [ENTER to advance, or describe]: ", i + 1, offset);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let note = input.trim();
        if !note.is_empty() {
            println!("  → {}", note);
        }
        clear(&iface, 0x80).await?;
    }

    // ── E: Brightness — does 0-255 work or just 0/non-zero? ─────────────
    println!("\n═══ E: Brightness levels on orange ═══\n");
    for &brightness in &[32u8, 64, 128, 255] {
        send_seg(&iface, 0x80, &orange_offsets, brightness).await?;
        prompt(&format!("E: Left orange at brightness {} — how bright?", brightness))?;
        clear(&iface, 0x80).await?;
    }

    println!("\nDone — all LEDs cleared.");
    Ok(())
}

/// Targeted touchstrip tests derived from the Traktor binary decompile.
///
/// Three hypotheses tested in order:
///
/// Test A — Left strip, all 25 interleaved simultaneously
///   prefix 0x80, offsets 93,95,97...141 (step=2, count=25) all lit at once
///
/// Test B — Right strip at decompile-stated absolute offsets 211–235
///   prefix 0x81, offsets 211,213,...259 (step=2) and flat 211-235
///
/// Test C — Segmented send: 0x80 covers bytes 0-117, 0x81 covers bytes 118-235
///   with right strip at relative offset 93 within the 0x81 segment (absolute 211)
///
/// Run: cargo run --bin touchstrip_decompile_test

use anyhow::{anyhow, Result};
use std::io::{self, Write};

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

fn prompt(label: &str) -> Result<bool> {
    print!("{} [ENTER=nothing, type anything=FOUND]: ", label);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(!input.trim().is_empty())
}

async fn send_offsets(iface: &nusb::Interface, prefix: u8, offsets: &[usize]) -> Result<()> {
    let mut buf = vec![0u8; 309];
    buf[0] = prefix;
    for &o in offsets {
        if 1 + o < buf.len() {
            buf[1 + o] = 255;
        }
    }
    iface.interrupt_out(0x03, buf).await.into_result()?;
    Ok(())
}

async fn clear(iface: &nusb::Interface) -> Result<()> {
    for prefix in [0x80u8, 0x81] {
        let mut buf = vec![0u8; 309];
        buf[0] = prefix;
        iface.interrupt_out(0x03, buf).await.into_result()?;
    }
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
    println!("║   Touchstrip Decompile-Derived Test                        ║");
    println!("║   LEDs sent first, then you report what you see            ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // ── Test A: Left strip, all 25 interleaved simultaneously ────────────
    println!("═══ Test A: Left strip — all 25 interleaved (93,95,...141), prefix 0x80 ═══\n");

    let left_interleaved: Vec<usize> = (0..25).map(|i| 93 + i * 2).collect();
    // Sanity: confirm pad 1 still works so we know hardware is responding
    send_offsets(&iface, 0x80, &[0]).await?; // pad 1 red channel
    if !prompt("A0: Pad 1 red (offset 0) lit as sanity check?")? {
        println!("  WARNING: pad 1 not responding — is another process claiming the interface?");
    }

    send_offsets(&iface, 0x80, &left_interleaved).await?;
    let found_a = prompt("A1: Left touchstrip lit? (offsets 93,95,97...141)")?;
    clear(&iface).await?;

    if found_a {
        println!("*** LEFT STRIP FOUND — interleaved 93,95,...141 with 0x80 ***");
        // Scan for which colour is which
        let blue_half: Vec<usize> = (0..13).map(|i| 93 + i * 2).collect();
        let orange_half: Vec<usize> = (13..25).map(|i| 93 + i * 2).collect();
        send_offsets(&iface, 0x80, &blue_half).await?;
        prompt("A2: Lower half (offsets 93–117) — what colour?")?;
        clear(&iface).await?;
        send_offsets(&iface, 0x80, &orange_half).await?;
        prompt("A3: Upper half (offsets 119–141) — what colour?")?;
        clear(&iface).await?;
    } else {
        println!("  No response on left interleaved.\n");
    }

    // ── Test B: Right strip at decompile absolute offsets 211–235 ─────────
    println!("\n═══ Test B: Right strip — flat offsets 211-235, prefix 0x81 ═══\n");

    let right_flat: Vec<usize> = (211..=235).collect();
    send_offsets(&iface, 0x81, &right_flat).await?;
    let found_b1 = prompt("B1: Right touchstrip lit? (offsets 211-235, prefix 0x81)")?;
    clear(&iface).await?;

    if found_b1 {
        println!("*** RIGHT STRIP FOUND — flat 211-235 with 0x81 ***");
    } else {
        // Also try interleaved at those positions
        let right_interleaved_high: Vec<usize> = (0..25).map(|i| 211 + i * 2).collect();
        send_offsets(&iface, 0x81, &right_interleaved_high).await?;
        let found_b2 = prompt("B2: Right touchstrip lit? (offsets 211,213,...259, prefix 0x81)")?;
        clear(&iface).await?;
        if found_b2 {
            println!("*** RIGHT STRIP FOUND — interleaved 211,213,...259 with 0x81 ***");
        } else {
            println!("  No response on right at 211+.\n");
        }
    }

    // ── Test C: Segmented send — 0x81 carries only bytes 118-235 ──────────
    // If Traktor sends 0x81 as a 118-byte report covering its own slice,
    // the right touchstrip at Traktor-internal 183 = relative offset 65 within
    // that segment, or at Traktor-internal 93 relative = absolute 211.
    // Send a SHORT 0x81 report (119 bytes: prefix + 118 data bytes) where the
    // touchstrip offsets are relative to the segment start.
    println!("\n═══ Test C: Segmented send — 0x81 as 119-byte report ═══\n");
    println!("  (Testing whether 0x81 uses its own 118-byte address space,");
    println!("   with right strip at relative offset 93 = same as left in 0x80)\n");

    // Short report: prefix + 118 bytes, touchstrip at relative offset 93 (same as left)
    let mut seg_buf = vec![0u8; 119]; // 1 prefix + 118 data
    seg_buf[0] = 0x81;
    for i in 0..25usize {
        seg_buf[1 + 93 + i] = 255; // relative offsets 93-117 within this segment
    }
    iface.interrupt_out(0x03, seg_buf).await.into_result()?;
    let found_c1 = prompt("C1: Right touchstrip lit? (0x81 segment, relative offsets 93-117)")?;
    // clear
    let mut clear_seg = vec![0u8; 119];
    clear_seg[0] = 0x81;
    iface.interrupt_out(0x03, clear_seg).await.into_result()?;

    if found_c1 {
        println!("*** RIGHT STRIP FOUND — segmented 0x81, relative offsets 93-117 ***");
    }

    // Also try interleaved relative offsets in the segment
    let mut seg_buf2 = vec![0u8; 119];
    seg_buf2[0] = 0x81;
    for i in 0..25usize {
        let rel = 93 + i * 2;
        if 1 + rel < seg_buf2.len() {
            seg_buf2[1 + rel] = 255;
        }
    }
    iface.interrupt_out(0x03, seg_buf2).await.into_result()?;
    let found_c2 = prompt("C2: Right touchstrip lit? (0x81 segment, relative interleaved 93,95,...141)")?;
    let mut clear_seg2 = vec![0u8; 119];
    clear_seg2[0] = 0x81;
    iface.interrupt_out(0x03, clear_seg2).await.into_result()?;

    if found_c2 {
        println!("*** RIGHT STRIP FOUND — segmented 0x81, relative interleaved ***");
    }

    if !found_a && !found_b1 && !found_c1 && !found_c2 {
        println!("\n  No response in any decompile-derived test.");
        println!("  Remaining option: USB capture of Traktor Pro driving the touchstrips.");
    }

    clear(&iface).await?;
    println!("\nDone — all LEDs cleared.");
    Ok(())
}

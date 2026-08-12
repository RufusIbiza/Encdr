/// Mixer LED sweep / interactive mapper.
///
/// Two modes:
///
/// Fast sweep (default):
///   Cycles rapidly through (prefix, offset) pairs in priority order.
///   Press ENTER the moment you see a mixer LED light up to record the address.
///
/// Interactive map (--interactive):
///   Steps through each address one at a time, holding the LED on until you
///   press ENTER. Use this to methodically identify every mixer LED.
///   Optionally restrict the range with --range PREFIX:START:END
///   e.g. --range 82:44:72
///
/// Flags:
///   --no-hs          Skip [0xf3, 0x01] handshake
///   --delay N        ms per address in fast mode (default 150)
///   --interactive    Step one address at a time, wait for ENTER
///   --range P:S:E    Only sweep prefix P (hex), offsets S..=E
use anyhow::Result;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

const S8_VENDOR_ID: u16 = 0x17cc;
const S8_PRODUCT_ID: u16 = 0x1370;

#[tokio::main]
async fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let no_handshake  = raw_args.iter().any(|a| a == "--no-hs");
    let interactive   = raw_args.iter().any(|a| a == "--interactive");
    let delay_ms: u64 = raw_args.windows(2)
        .find(|w| w[0] == "--delay")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(150);

    // Optional --range PREFIX:START:END  e.g. --range 82:44:72
    let range_filter: Option<(u8, usize, usize)> = raw_args.windows(2)
        .find(|w| w[0] == "--range")
        .and_then(|w| {
            let parts: Vec<&str> = w[1].split(':').collect();
            if parts.len() == 3 {
                let p = u8::from_str_radix(parts[0], 16).ok()?;
                let s: usize = parts[1].parse().ok()?;
                let e: usize = parts[2].parse().ok()?;
                Some((p, s, e))
            } else { None }
        });

    // ── build sweep list ─────────────────────────────────────────────────
    // Each entry: (prefix, data_offset, label)
    // data_offset = index into the 308 data bytes (i.e. byte [1+offset] in the 309-byte packet)
    let mut sweep: Vec<(u8, usize, &'static str)> = Vec::new();

    if let Some((p, s, e)) = range_filter {
        for off in s..=e { sweep.push((p, off, "range")); }
    } else {
        // 0x80 offsets 0–117: left deck  — SKIP (all in descriptor)
        // 0x81 offsets 0–117: right deck — SKIP (all in descriptor)
        //
        // 0x82: mixer section — NOT in descriptor yet.
        //   offsets  0–43: VU meters (11 LEDs × 4 channels, per binary analysis)
        //   offsets 44–53: FX assign inactive + snap/quant (confirmed working)
        //   offsets 54–72: unknown — binary predicts mic assign at 62–63
        for off in 0usize..=72 { sweep.push((0x82, off, "0x82")); }
    }

    let mode_str = if interactive { "INTERACTIVE (Enter to advance)" } else { &format!("FAST {}ms/address", delay_ms) };
    println!("S8 Mixer LED Sweep — {} addresses, {}", sweep.len(), mode_str);
    if no_handshake { println!("  Handshake: SKIPPED (--no-hs)"); }
    else             { println!("  Handshake: [0xf3, 0x01] sent once before sweep"); }
    if interactive {
        println!("  Each address stays ON until you press ENTER — note which button lit, then advance.");
        println!("Press ENTER to start.\n");
    } else {
        println!("Press ENTER to start, then press ENTER again the moment you see a mixer LED light.\n");
    }

    // Wait for initial Enter before opening USB
    let mut init_line = String::new();
    tokio::io::BufReader::new(tokio::io::stdin()).read_line(&mut init_line).await?;

    // ── open interface 5 once ────────────────────────────────────────────
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow::anyhow!("S8 not found"))?;
    let device = device_info.open()?;
    let iface = device.detach_and_claim_interface(5)?;

    if !no_handshake {
        println!("Sending [0xf3, 0x01] handshake...");
        iface.interrupt_out(0x03, vec![0xf3u8, 0x01]).await.into_result()?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // ── stdin watcher ────────────────────────────────────────────────────
    // Spawn a background task that sends a message on Enter.
    let (tx, mut rx) = mpsc::channel::<()>(4);
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = tokio::io::BufReader::new(stdin);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap_or(0) == 0 { break; }
            if tx.send(()).await.is_err() { break; }
        }
    });

    // ── sweep ─────────────────────────────────────────────────────────────
    let mut hit_idxs: Vec<(usize, String)> = Vec::new();

    if interactive {
        println!("Interactive map — LED ON, press ENTER to note what's lit and advance:");
        println!("(type a label before pressing ENTER to annotate, or just press ENTER to skip)\n");
        use std::io::Write;

        for (idx, &(prefix, offset, tier)) in sweep.iter().enumerate() {
            let abs = match prefix {
                0x80 => offset,
                0x81 => 118 + offset,
                0x82 => 236 + offset,
                _    => offset,
            };

            print!("[{:>3}/{}] {} 0x{:02x} off={:>3} abs={:>3}  — lit, press ENTER (type label+enter to annotate): ",
                idx + 1, sweep.len(), tier, prefix, offset, abs);
            std::io::stdout().flush().ok();

            // LED ON
            let mut buf_on = vec![0u8; 309];
            buf_on[0] = prefix;
            buf_on[1 + offset] = 0x7f;
            let _ = iface.interrupt_out(0x03, buf_on.clone()).await.into_result();

            // Wait for Enter (blocking read in sync wrapper since interactive = one at a time)
            let label = {
                let mut l = String::new();
                tokio::io::BufReader::new(tokio::io::stdin()).read_line(&mut l).await?;
                l.trim().to_string()
            };
            if !label.is_empty() {
                hit_idxs.push((idx, label.clone()));
                println!("  → annotated: \"{}\"  (prefix=0x{:02x} off={} abs={})", label, prefix, offset, abs);
            }

            // LED OFF
            let mut buf_off = vec![0u8; 309];
            buf_off[0] = prefix;
            let _ = iface.interrupt_out(0x03, buf_off).await.into_result();
        }
    } else {
        println!("Sweeping — press ENTER when you see a mixer LED light:");
        let mut idx = 0;
        while idx < sweep.len() {
            let (prefix, offset, tier) = sweep[idx];
            let abs = match prefix {
                0x80 => offset,
                0x81 => 118 + offset,
                0x82 => 236 + offset,
                _    => offset,
            };

            print!("\r  [{:>3}/{}] {} 0x{:02x} off={:>3} abs={:>3}   ",
                idx + 1, sweep.len(), tier, prefix, offset, abs);
            use std::io::Write;
            std::io::stdout().flush().ok();

            // LED ON
            let mut buf_on = vec![0u8; 309];
            buf_on[0] = prefix;
            buf_on[1 + offset] = 0x7f;
            let _ = iface.interrupt_out(0x03, buf_on).await.into_result();

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                _ = rx.recv() => {
                    println!("\n  *** HIT idx={} prefix=0x{:02x} offset={} abs={} ***",
                        idx, prefix, offset, abs);
                    hit_idxs.push((idx, format!("0x{:02x}:{}", prefix, offset)));
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    while rx.try_recv().is_ok() {}
                    println!("Continuing — press ENTER again for next hit...");
                }
            }

            // LED OFF
            let mut buf_off = vec![0u8; 309];
            buf_off[0] = prefix;
            let _ = iface.interrupt_out(0x03, buf_off).await.into_result();

            idx += 1;
        }
    }

    println!("\n\nSweep complete.");
    if hit_idxs.is_empty() {
        if !interactive {
            println!("No hits recorded.");
            println!("Suggestion: re-run with --no-hs to try without handshake.");
        }
    } else {
        println!("Results ({}):", hit_idxs.len());
        for (i, label) in &hit_idxs {
            let (prefix, offset, tier) = sweep[*i];
            let abs = match prefix { 0x80 => offset, 0x81 => 118+offset, _ => 236+offset };
            println!("  {} idx={} tier={} prefix=0x{:02x} offset={} abs={}", label, i, tier, prefix, offset, abs);
        }
    }

    Ok(())
}

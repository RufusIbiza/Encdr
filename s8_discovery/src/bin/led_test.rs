use anyhow::{anyhow, Result};
use std::io::{self, BufRead};
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
        "handshake" => {
            println!("Sending [0xf3, 0x01]...");
            send_raw_report_custom_size(0xf3, &[0x01], 2).await?;
        }
        "f3_test" => {
            // Test 0xf3 as a control byte rather than a handshake.
            // [0xf3, 0x00] — does this turn OFF the mixer button LEDs?
            // [0xf3, 0x01, ...N bytes of channel data] — does format matter?
            let subcmd = args.get(2).map(|s| s.as_str()).unwrap_or("off");
            match subcmd {
                "off" => {
                    println!("Sending [0xf3, 0x00] — expect mixer LEDs to turn off if 0xf3 controls them...");
                    send_raw_report_custom_size(0xf3, &[0x00], 2).await?;
                }
                "on" => {
                    println!("Sending [0xf3, 0x01] (reference, all on)...");
                    send_raw_report_custom_size(0xf3, &[0x01], 2).await?;
                }
                "ch" => {
                    // Send [0xf3, ch0, ch1, ..., ch10] = 12 bytes, one brightness per channel
                    // Try setting channel 0 only to see if it lights one specific button
                    let ch: u8 = args.get(3).map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
                    let val: u8 = args.get(4).map(|s| s.parse().unwrap_or(127)).unwrap_or(127);
                    let mut data = vec![0u8; 11]; // 11 channels
                    if (ch as usize) < 11 { data[ch as usize] = val; }
                    println!("Sending [0xf3] + 11 channel bytes, ch{} = {} ...", ch, val);
                    send_raw_report_custom_size(0xf3, &data, 12).await?;
                }
                "all" => {
                    // [0xf3, 0x7f x 11] = all channels max brightness
                    println!("Sending [0xf3] + 11 bytes of 0x7f ...");
                    send_raw_report_custom_size(0xf3, &[0x7fu8; 11], 12).await?;
                }
                _ => println!("f3_test: off | on | ch <n> [val] | all"),
            }
        }
        "f3_raw" => {
            // Send [0xf3, byte1, byte2, ..., byteN] with explicit hex bytes.
            // Usage: led_test f3_raw <hex_byte>...
            // e.g.:  led_test f3_raw 7f 7f 00 00 00 00 00 00 00 00 00
            //        (byte[1]=0x7f=sw_enable, byte[2]=0x7f=ch0 on, rest off)
            let data: Vec<u8> = args[2..].iter()
                .filter_map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .collect();
            let total = data.len() + 1;
            println!("Sending [0xf3, {}] ({} bytes)...",
                data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "),
                total);
            send_raw_report_custom_size(0xf3, &data, total).await?;
        }
        "bulk_test" => {
            // Interface 6, endpoint 0x04 — vendor-specific bulk OUT (512-byte max).
            // Never been tested. Try sending 0xf3-style or 0x80-style packets here.
            let prefix = args.get(2)
                .map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0x80))
                .unwrap_or(0x80);
            let size: usize = args.get(3).map(|s| s.parse().unwrap_or(309)).unwrap_or(309);
            println!("Bulk EP 0x04 (iface 6): sending {} bytes prefix 0x{:02x}...", size, prefix);
            let mut buf = vec![0u8; size];
            buf[0] = prefix;
            // Set a mid-range byte bright so something should light if this endpoint works
            if size > 30 { buf[30] = 0x7f; }
            let mut devices = nusb::list_devices()?;
            let device_info = devices
                .find(|info| info.vendor_id() == 0x17cc && info.product_id() == 0x1370)
                .ok_or_else(|| anyhow::anyhow!("S8 not found"))?;
            let device = device_info.open()?;
            let iface = device.detach_and_claim_interface(6)?;
            match iface.bulk_out(0x04, buf).await.into_result() {
                Ok(_) => println!("  Transfer OK"),
                Err(e) => println!("  Transfer error: {}", e),
            }
        }
        "test_mixer" => {
            // Confirmed from Traktor decompiled source (sub_14067fac0 / S8PortDescriptor body).
            // Absolute LED indices → 0x81 packet offset = abs - 118, 0x82 offset = abs - 236.
            // Handshake ([0xf3, 0x01]) must be sent before this.
            println!("Testing confirmed mixer LED offsets (send handshake first)...");

            // FX Assign 1, channel 1 (absolute LED 149 → 0x81 offset 31)
            println!("  Ch1 FX Assign 1 active LED (0x81 offset 31)...");
            send_led_report(0x81, vec![(31, 127)]).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;

            // FX Assign 1, channel 3 (absolute LED 147 → 0x81 offset 29)
            println!("  Ch3 FX Assign 1 active LED (0x81 offset 29)...");
            send_led_report(0x81, vec![(29, 127)]).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Snap (absolute LED 151 → 0x81 offset 33)
            println!("  Snap active LED (0x81 offset 33)...");
            send_led_report(0x81, vec![(33, 127)]).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;

            // VU ch3 bottom LED (absolute LED 236 → 0x82 offset 0)
            println!("  Ch3 VU bottom (0x82 offset 0)...");
            send_led_report(0x82, vec![(0, 127)]).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;

            // VU ch1 bottom LED (absolute LED 247 → 0x82 offset 11)
            println!("  Ch1 VU bottom (0x82 offset 11)...");
            send_led_report(0x82, vec![(11, 127)]).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;

            println!("  Test complete.");
        }
        "sweep_mixer" => {
            // Sweep 0x81 offsets 28–57 one at a time to locate CUE, filter_on, deck_assign.
            // Known assignments in this range:
            //   28=mic.assign.1, 29=ch3_fx1, 30=ch3_fx2, 31=ch1_fx1, 32=ch1_fx2, 33=snap
            //   50=mic.assign.2, 53=quant, 54=ch2_fx1, 55=ch2_fx2, 56=ch4_fx1, 57=ch4_fx2
            // Unknown: offsets 34–52 (absolute LEDs 152–170) — likely CUE/filter_on/deck_assign
            let start: usize = args.get(2).map(|s| s.parse().unwrap_or(28)).unwrap_or(28);
            let end: usize = args.get(3).map(|s| s.parse().unwrap_or(58)).unwrap_or(58);
            println!("Sweeping 0x81 offsets {}–{} (press Enter between each)...", start, end);
            for offset in start..end {
                println!("  0x81 offset {} (absolute LED {}) — look for lit button, then press Enter", offset, 118 + offset);
                send_led_report(0x81, vec![(offset, 127)]).await?;
                let mut line = String::new();
                io::stdin().lock().read_line(&mut line)?;
                send_led_report(0x81, vec![(offset, 0)]).await?;
            }
            println!("Sweep complete.");
        }
        "wakeup" => {
            println!("Sending wakeup command (0x01)...");
            send_raw_report_custom_size(0x01, &[], 1).await?;
        }
        "cue_blink" => {
            // Trace from sub_140c9ba40: slot 0x18→LED 119 (CueA), 0x19→118 (CueB),
            //                           0x1a→127 (CueC), 0x1b→126 (CueD).
            // sub_140c9e870: all are SIMPLE type, 0x81 packet, offset = abs_led - 118.
            // CueA=offset 1, CueB=offset 0, CueC=offset 9, CueD=offset 8.
            //
            // Handshake and LED packets sent in the SAME interface session to avoid
            // the interface-release issue from running them as separate invocations.
            let channel: usize = args.get(2).map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
            let cue_offsets = [1usize, 0, 9, 8]; // A, B, C, D → absolute LEDs 119, 118, 127, 126
            let offset = cue_offsets[channel.min(3)];
            let label = ["A", "B", "C", "D"][channel.min(3)];
            println!("CUE {} blink: 0x81 offset {} (abs LED {}) — handshake + LED in single session", label, offset, 118 + offset);
            cue_blink_session(offset).await?;
        }
        "mixer_blink" => {
            // Blink an arbitrary 0x81 offset in a single session with the handshake.
            // Usage: led_test mixer_blink <offset> [value]
            if args.len() < 3 {
                println!("Usage: led_test mixer_blink <offset> [value=127]");
                return Ok(());
            }
            let offset: usize = args[2].parse()?;
            let value: u8 = args.get(3).map(|s| s.parse().unwrap_or(127)).unwrap_or(127);
            println!("Blinking 0x81 offset {} (abs LED {}) with value {}...", offset, 118 + offset, value);
            cue_blink_session_value(offset, value).await?;
        }
        "brute_mixer" => {
            // Brute-force prefix bytes 0x83–0xff (skipping 0x80/0x81 deck prefixes and 0x82
            // which we know), with same-session handshake. Tests one offset (default 48 =
            // confirmed mixer snap LED) to verify the session is alive, then lights offset 1
            // across all prefixes. Press ENTER when a *new* LED lights.
            // Usage: led_test brute_mixer [offset=1]
            let probe_offset: usize = args.get(2).map(|s| s.parse().unwrap_or(1)).unwrap_or(1);
            let mut devices = nusb::list_devices()?;
            let device_info = devices
                .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
                .ok_or_else(|| anyhow!("S8 not found"))?;
            let device = device_info.open()?;
            let iface = device.detach_and_claim_interface(5)?;
            // 0x82 packets only work AFTER the handshake — sending them before hangs.
            // Handshake first, then blink snap by turning all known LEDs off, lighting
            // only snap, then off again. Visible as: 10 LEDs off → snap on → snap off.
            iface.interrupt_out(0x03, vec![0xf3u8, 0x01]).await.into_result()?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            // All known 0x82 button LEDs off (FX assign, snap, quant)
            let mut all_off = vec![0u8; 309]; all_off[0] = 0x82;
            iface.interrupt_out(0x03, all_off.clone()).await.into_result()?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            // Snap only on
            let mut snap_on = all_off.clone(); snap_on[49] = 0x7f;
            iface.interrupt_out(0x03, snap_on).await.into_result()?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            // Snap off
            iface.interrupt_out(0x03, all_off.clone()).await.into_result()?;
            tokio::time::sleep(Duration::from_millis(200)).await;
            println!("Snap should have blinked. Sweeping prefixes 0x83–0xff, offset {}...", probe_offset);
            println!("Press ENTER when you see an *unexpected* LED light.");
            let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(4);
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut r = tokio::io::BufReader::new(tokio::io::stdin());
                loop {
                    let mut l = String::new();
                    if r.read_line(&mut l).await.unwrap_or(0) == 0 { break; }
                    if tx.send(()).await.is_err() { break; }
                }
            });
            for prefix in 0x83u8..=0xff {
                if prefix == 0xf3 { continue; } // skip known group toggle
                print!("\r  prefix 0x{:02x} offset {}   ", prefix, probe_offset);
                use std::io::Write; std::io::stdout().flush().ok();
                let mut buf = vec![0u8; 309]; buf[0] = prefix; buf[1 + probe_offset] = 0x7f;
                let _ = iface.interrupt_out(0x03, buf).await.into_result();
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(120)) => {}
                    _ = rx.recv() => {
                        println!("\nHIT: prefix 0x{:02x} offset {}", prefix, probe_offset);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        while rx.try_recv().is_ok() {}
                    }
                }
                let mut off = vec![0u8; 309]; off[0] = prefix;
                let _ = iface.interrupt_out(0x03, off).await.into_result();
            }
            println!("\nDone.");
        }
        "vu_test" => {
            // VU meters: 0x82 offsets 0–43, 11 segments per channel.
            // Binary predicts: ch3=0–10, ch1=11–21, ch2=22–32, ch4=33–43.
            // Test by filling all 11 segments of one channel at once (value 0x7f).
            // Usage: led_test vu_test <channel 0–3> [value=127]
            // channel order: 0=ch3(DeckC), 1=ch1(DeckA), 2=ch2(DeckB), 3=ch4(DeckD)
            let ch: usize = args.get(2).map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
            let val: u8 = args.get(3).map(|s| s.parse().unwrap_or(127)).unwrap_or(127);
            let base_offset = ch.min(3) * 11;
            println!("VU test: channel {} (0x82 offsets {}–{}) = {}",
                ch, base_offset, base_offset + 10, val);
            let mut devices = nusb::list_devices()?;
            let device_info = devices
                .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
                .ok_or_else(|| anyhow!("S8 not found"))?;
            let device = device_info.open()?;
            let iface = device.detach_and_claim_interface(5)?;
            iface.interrupt_out(0x03, vec![0xf3u8, 0x01]).await.into_result()?;
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut buf = vec![0u8; 309];
            buf[0] = 0x82;
            for i in 0..11 { buf[1 + base_offset + i] = val; }
            iface.interrupt_out(0x03, buf.clone()).await.into_result()?;
            println!("Holding for 3s...");
            tokio::time::sleep(Duration::from_secs(3)).await;
            buf[0] = 0x82;
            for i in 0..11 { buf[1 + base_offset + i] = 0; }
            iface.interrupt_out(0x03, buf).await.into_result()?;
        }
        "midi_sysex" => {
            // Send MIDI SysEx via interface 3, EP 0x02 (USB MIDI class bulk).
            // Bytes given are the payload; 0xF0 and 0xF7 are added automatically.
            // e.g.: led_test midi_sysex 00 21 09 77 7f
            //       → [0xF0, 0x00, 0x21, 0x09, 0x77, 0x7f, 0xF7]
            let payload: Vec<u8> = args[2..].iter()
                .filter_map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .collect();
            if payload.is_empty() {
                println!("Usage: led_test midi_sysex <hex_byte>...");
                return Ok(());
            }
            println!("Sending MIDI SysEx [0xf0 {} f7] on interface 3 EP 0x02...",
                payload.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));
            send_midi_sysex(&payload).await?;
        }
        "midi_raw" => {
            // Send raw bytes (no USB MIDI framing) to interface 3, EP 0x02.
            // Useful if the device doesn't use standard USB MIDI class framing.
            let data: Vec<u8> = args[2..].iter()
                .filter_map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .collect();
            if data.is_empty() {
                println!("Usage: led_test midi_raw <hex_byte>...");
                return Ok(());
            }
            println!("Sending {} raw bytes to interface 3 EP 0x02: {}",
                data.len(),
                data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));
            let mut devices = nusb::list_devices()?;
            let device_info = devices
                .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
                .ok_or_else(|| anyhow!("S8 not found"))?;
            let device = device_info.open()?;
            let iface = device.detach_and_claim_interface(3)?;
            match iface.bulk_out(0x02, data).await.into_result() {
                Ok(_) => println!("  Transfer OK"),
                Err(e) => println!("  Transfer error: {}", e),
            }
        }
        _ => print_usage(),
    }

    Ok(())
}

/// Send MIDI SysEx using USB MIDI Class encoding (4-byte packets) on interface 3, EP 0x02.
async fn send_midi_sysex(payload: &[u8]) -> Result<()> {
    let sysex: Vec<u8> = std::iter::once(0xF0u8)
        .chain(payload.iter().copied())
        .chain(std::iter::once(0xF7u8))
        .collect();

    // Encode as USB MIDI class packets (CIN + 3 MIDI bytes each).
    let mut usb_packets: Vec<u8> = Vec::new();
    let n = sysex.len();
    let mut i = 0;
    while i < n {
        let rem = n - i;
        if rem > 3 {
            usb_packets.extend_from_slice(&[0x04, sysex[i], sysex[i+1], sysex[i+2]]);
            i += 3;
        } else if rem == 3 {
            usb_packets.extend_from_slice(&[0x07, sysex[i], sysex[i+1], sysex[i+2]]);
            i += 3;
        } else if rem == 2 {
            usb_packets.extend_from_slice(&[0x06, sysex[i], sysex[i+1], 0x00]);
            i += 2;
        } else {
            usb_packets.extend_from_slice(&[0x05, sysex[i], 0x00, 0x00]);
            i += 1;
        }
    }

    println!("  USB MIDI packets: {}",
        usb_packets.chunks(4)
            .map(|c| format!("[{:02x} {:02x} {:02x} {:02x}]", c[0], c[1], c[2], c[3]))
            .collect::<Vec<_>>().join(" "));

    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;
    let device = device_info.open()?;
    let iface = device.detach_and_claim_interface(3)?;
    match iface.bulk_out(0x02, usb_packets).await.into_result() {
        Ok(_) => println!("  Transfer OK"),
        Err(e) => println!("  Transfer error: {}", e),
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
    if total_size > 1 && !data.is_empty() {
        let copy_len = std::cmp::min(data.len(), total_size - 1);
        buffer[1..1+copy_len].copy_from_slice(&data[..copy_len]);
    }

    interface.interrupt_out(0x03, buffer).await.into_result()?;
    Ok(())
}

/// Sends the Traktor Mode handshake then blinks the given 0x81 data offset,
/// all within the same interface claim (avoids handshake-state loss on reconnect).
async fn cue_blink_session(offset: usize) -> Result<()> {
    cue_blink_session_value(offset, 127).await
}

async fn cue_blink_session_value(offset: usize, value: u8) -> Result<()> {
    let mut devices = nusb::list_devices()?;
    let device_info = devices
        .find(|info| info.vendor_id() == S8_VENDOR_ID && info.product_id() == S8_PRODUCT_ID)
        .ok_or_else(|| anyhow!("S8 not found"))?;

    let device = device_info.open()?;
    let iface = device.detach_and_claim_interface(5)?;

    // Handshake — enables Traktor Mode LED control for mixer section.
    println!("  Sending handshake [0xf3, 0x01]...");
    iface.interrupt_out(0x03, vec![0xf3u8, 0x01]).await.into_result()?;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // LED ON: full 309-byte packet, prefix 0x81, single offset lit.
    let mut buf_on = vec![0u8; 309];
    buf_on[0] = 0x81;
    buf_on[1 + offset] = value;
    println!("  LED ON (0x81 offset {}, value {})...", offset, value);
    iface.interrupt_out(0x03, buf_on).await.into_result()?;

    println!("  Waiting 2s — look for lit CUE button...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // LED OFF: same packet, byte set to 0.
    let mut buf_off = vec![0u8; 309];
    buf_off[0] = 0x81;
    println!("  LED OFF...");
    iface.interrupt_out(0x03, buf_off).await.into_result()?;

    println!("  Done.");
    Ok(())
}

fn print_usage() {
    println!("S8 LED Test Tool");
    println!("Usage:");
    println!("  led_test handshake                              - Enable Traktor Mode (required first)");
    println!("  led_test cue_blink [channel]                    - Blink CUE A/B/C/D (0-3) in single session with handshake");
    println!("  led_test mixer_blink <offset> [value]           - Blink 0x81 offset in single session with handshake");
    println!("  led_test test_mixer                             - Test confirmed mixer LED offsets");
    println!("  led_test sweep_mixer [start] [end]              - Sweep 0x81 offsets to find CUE/filter LEDs");
    println!("  led_test set <prefix_hex> <offset> <value>      - Set a single LED");
    println!("  led_test sweep <prefix_hex> [start]             - Sweep through offsets");
    println!("  led_test all_off                                - Turn off all LEDs");
    println!("  led_test all_on <prefix_hex>                    - Turn on all LEDs for a prefix");
    println!("  led_test brute_prefix [offset]                  - Brute-force through Report IDs");
    println!("  led_test wakeup                                 - Send device wakeup (HID enable)");
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
    let interface = device.detach_and_claim_interface(5)?;

    // Standard S8 LED report: 1-byte Report ID + 308-byte data = 309 bytes total
    let mut buffer = vec![0u8; 309];
    buffer[0] = prefix;  // Report ID
    let copy_len = std::cmp::min(data.len(), 308);
    buffer[1..1+copy_len].copy_from_slice(&data[..copy_len]);

    println!("Sending LED report with Report ID 0x{:02x}", prefix);

    match interface.interrupt_out(0x03, buffer).await.into_result() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("Failed to send to 0x03: {}", e))
    }
}

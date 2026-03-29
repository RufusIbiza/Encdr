use std::time::Duration;
use encdr::{Encdr, EncdrConfig, Event};

fn main() {
    // Initialize
    let mut encdr = Encdr::new(EncdrConfig::default())
        .expect("Failed to initialize encdr");

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         S8 Input Monitor - Encdr Descriptor Test           ║");
    println!("║                                                            ║");
    println!("║  This verifies the S8 descriptor is correctly mapping      ║");
    println!("║  USB input packets to control names.                       ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Scanning for S8...");

    // Connect to devices
    match encdr.scan() {
        Ok(ids) => {
            if ids.is_empty() {
                println!("❌ No devices found!");
                return;
            }
            for id in &ids {
                if let Some(desc) = encdr.device_descriptor(*id) {
                    println!("✓ Found: {} [{}]", desc.name, id);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Scan error: {}", e);
            return;
        }
    }

    println!("\n📡 Listening for S8 inputs... (move controls, Ctrl+C to quit)\n");

    let events = encdr.events().clone();
    let mut last_control: Option<String> = None;
    let mut event_count = 0;

    loop {
        match events.recv_timeout(Duration::from_secs(2)) {
            Ok(event) => {
                event_count += 1;
                let control_name = match &event {
                    Event::Button { device, name, pressed } => {
                        let state = if *pressed { "pressed" } else { "released" };
                        Some(format!("{}: {} ({})", device, name, state))
                    }
                    Event::Slider { device, name, value } => {
                        Some(format!("{}: {} = {:.3}", device, name, value))
                    }
                    Event::Encoder { device, name, delta } => {
                        Some(format!("{}: {} delta={:+}", device, name, delta))
                    }
                    Event::EncoderFine { device, name, delta } => {
                        Some(format!("{}: {} delta={:+.4}", device, name, delta))
                    }
                    Event::Touch { device, name, touched } => {
                        let state = if *touched { "touched" } else { "released" };
                        Some(format!("{}: {} {}", device, name, state))
                    }
                    Event::Grid { device, name, index, pressure } => {
                        Some(format!(
                            "{}: {} [pad {}] pressure={:.2}",
                            device, name, index, pressure
                        ))
                    }
                    _ => None,
                };

                if let Some(control) = control_name {
                    if last_control.as_ref() != Some(&control) {
                        println!("  {}", control);
                        last_control = Some(control);
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Re-scan periodically for hotplug
                encdr.scan().ok();
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                println!("\n❌ Event channel closed.");
                break;
            }
        }
    }

    println!("\n✓ Processed {} events", event_count);
}

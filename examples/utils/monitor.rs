use std::time::Duration;

use encdr::{Encdr, EncdrConfig, Event};

fn main() {
    // Initialize
    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("=== Encdr Event Monitor ===");
    println!("Scanning for devices...\n");

    // Connect to any detected devices
    match encdr.scan() {
        Ok(ids) if ids.is_empty() => {
            println!("No devices found. Waiting for hotplug...");
        }
        Ok(ids) => {
            for id in &ids {
                if let Some(desc) = encdr.device_descriptor(*id) {
                    println!("Connected: {} [{}]", desc.name, id);
                }
            }
            println!();
        }
        Err(e) => {
            eprintln!("Scan error: {}", e);
            return;
        }
    }

    println!("Listening for events (Ctrl+C to quit)...\n");

    let events = encdr.events().clone();

    loop {
        match events.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => print_event(&event),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Periodically re-scan for new devices
                encdr.scan().ok();
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                println!("Event channel closed.");
                break;
            }
        }
    }
}

fn print_event(event: &Event) {
    match event {
        Event::DeviceConnected { id, descriptor } => {
            println!("[CONNECT]    {} [{}]", descriptor.name, id);
        }
        Event::DeviceDisconnected { id } => {
            println!("[DISCONNECT] [{}]", id);
        }
        Event::Button {
            device,
            name,
            pressed,
        } => {
            let state = if *pressed { "PRESSED " } else { "RELEASED" };
            println!("[BUTTON]     {} {:16} {}", device, name, state);
        }
        Event::Slider {
            device,
            name,
            value,
        } => {
            let bar_len = (*value * 30.0) as usize;
            let bar: String = "█".repeat(bar_len) + &"░".repeat(30 - bar_len);
            println!("[SLIDER]     {} {:16} {:.3} |{}|", device, name, value, bar);
        }
        Event::Encoder {
            device,
            name,
            delta,
        } => {
            let arrow = if *delta > 0 { "→" } else { "←" };
            println!(
                "[ENCODER]    {} {:16} {:+3} {}",
                device, name, delta, arrow
            );
        }
        Event::EncoderFine {
            device,
            name,
            delta,
        } => {
            let arrow = if *delta > 0.0 { "→" } else { "←" };
            println!(
                "[ENC_FINE]   {} {:16} {:+.4} {}",
                device, name, delta, arrow
            );
        }
        Event::Touch {
            device,
            name,
            touched,
        } => {
            let state = if *touched { "TOUCH  " } else { "RELEASE" };
            println!("[TOUCH]      {} {:16} {}", device, name, state);
        }
        Event::Grid {
            device,
            name,
            index,
            pressure,
        } => {
            println!(
                "[GRID]       {} {:16} pad={} pressure={:.2}",
                device, name, index, pressure
            );
        }
    }
}

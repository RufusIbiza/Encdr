use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::Sender;
use futures_lite::future::block_on;
use nusb::transfer::RequestBuffer;

use crate::core::descriptor::*;
use crate::core::event::{DeviceId, Event};
use crate::core::led::LedValue;
use crate::device::hooks::{NoopHook, PacketHook};
use crate::device::led_builder::LedBuilder;
use crate::device::parser::PacketParser;
use crate::screen::ScreenManager;

/// Commands sent from the main thread to a device thread.
pub enum DeviceCmd {
    SetLed { name: String, value: LedValue },
    SetLedInGroup { group: String, name: String, value: LedValue },
    SetLedStrip { name: String, values: Vec<u8> },
    SetLedStripInGroup { group: String, name: String, values: Vec<u8> },
    SubmitScreen { screen: String, pixels: Vec<u8>, format: PixelFormat },
    Disconnect,
}

/// Manages a single connected device's I/O thread.
pub struct DeviceHandle {
    pub device_id: DeviceId,
    pub descriptor: Arc<DeviceDescriptor>,
    cmd_tx: async_channel::Sender<DeviceCmd>,
    thread: Option<thread::JoinHandle<()>>,
}

impl DeviceHandle {
    /// Spawn a device I/O thread. Returns a handle for sending commands.
    pub fn spawn(
        device_id: DeviceId,
        descriptor: Arc<DeviceDescriptor>,
        usb_info: &nusb::DeviceInfo,
        names: HashMap<String, &'static str>,
        event_tx: Sender<Event>,
        gpu: Option<Arc<crate::screen::GpuContext>>,
    ) -> Result<Self, crate::core::error::EncdrError> {
        let (cmd_tx, cmd_rx) = async_channel::bounded(256);

        let usb_device = usb_info.open()?;
        let desc = descriptor.clone();
        let id = device_id;

        let thread = thread::Builder::new()
            .name(format!("encdr-{}", descriptor.name.replace(' ', "-").to_lowercase()))
            .spawn(move || {
                run_device(id, desc, usb_device, names, cmd_rx, event_tx, gpu);
            })
            .map_err(|e| crate::core::error::EncdrError::Io(e))?;

        Ok(Self {
            device_id,
            descriptor,
            cmd_tx,
            thread: Some(thread),
        })
    }

    pub fn send(&self, cmd: DeviceCmd) {
        // try_send is non-blocking: safe to call from any (non-async) thread.
        self.cmd_tx.try_send(cmd).ok();
    }

    pub fn disconnect(mut self) {
        self.cmd_tx.try_send(DeviceCmd::Disconnect).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        self.cmd_tx.try_send(DeviceCmd::Disconnect).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

/// Main device I/O loop. Runs on a dedicated thread.
fn run_device(
    device_id: DeviceId,
    descriptor: Arc<DeviceDescriptor>,
    usb_device: nusb::Device,
    names: HashMap<String, &'static str>,
    cmd_rx: async_channel::Receiver<DeviceCmd>,
    event_tx: Sender<Event>,
    gpu: Option<Arc<crate::screen::GpuContext>>,
) {
    // Claim interfaces
    let mut interfaces: HashMap<String, nusb::Interface> = HashMap::new();

    for iface_desc in &descriptor.interfaces {
        match usb_device.detach_and_claim_interface(iface_desc.number) {
            Ok(iface) => {
                tracing::info!(
                    "Claimed interface {} ('{}')",
                    iface_desc.number,
                    iface_desc.id
                );
                interfaces.insert(iface_desc.id.clone(), iface);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to claim interface {} ({}): {}",
                    iface_desc.number,
                    iface_desc.id,
                    e
                );
                return;
            }
        }
    }

    // Determine the control interface for input reading
    let control_iface_id = descriptor
        .input_packets
        .first()
        .map(|p| p.interface.clone())
        .unwrap_or_else(|| "control".to_string());

    let Some(control_iface) = interfaces.get(&control_iface_id) else {
        tracing::error!("Control interface '{}' not found", control_iface_id);
        return;
    };

    // Find the input endpoint address
    let input_ep = descriptor
        .interface_by_id(&control_iface_id)
        .and_then(|i| i.endpoints.ep_in.as_ref())
        .map(|ep| ep.address.0 as u8)
        .unwrap_or(0x81);

    // Build LED controllers (one per LED group)
    let led_builders: Vec<LedBuilder> = descriptor
        .leds
        .iter()
        .filter_map(|led_desc| {
            let iface = descriptor.interface_by_id(&led_desc.interface)?;
            Some(LedBuilder::new(led_desc, iface))
        })
        .collect();

    // Build screen managers
    let mut screen_managers: HashMap<String, ScreenManager> = HashMap::new();
    for screen_desc in &descriptor.screens {
        let sm = ScreenManager::new(screen_desc, gpu.clone());
        screen_managers.insert(screen_desc.name.clone(), sm);
    }

    // Build packet parser
    let mut parser = PacketParser::new(device_id, &descriptor, names);
    let mut hook: Box<dyn PacketHook> = Box::new(NoopHook);
    let mut led_builders = led_builders;

    // Send blank screen on connect (splash)
    for (screen_name, _sm) in &screen_managers {
        if let Some(screen_iface_id) = descriptor
            .screens
            .iter()
            .find(|s| s.name == *screen_name)
            .map(|s| &s.interface)
        {
            if let Some(iface) = interfaces.get(screen_iface_id) {
                let screen_desc = descriptor
                    .screens
                    .iter()
                    .find(|s| s.name == *screen_name)
                    .unwrap();
                let ep = descriptor
                    .interface_by_id(screen_iface_id)
                    .and_then(|i| i.endpoints.out.as_ref())
                    .map(|ep| ep.address.0 as u8)
                    .unwrap_or(0x02);

                let blank = vec![0u8; screen_desc.byte_size()];
                let blit_buf = crate::screen::protocol::build_full_blit(screen_desc, &blank);
                tracing::info!(
                    "Sending splash screen: {} bytes to ep 0x{:02x} on interface '{}'",
                    blit_buf.len(),
                    ep,
                    screen_iface_id
                );
                block_on(async {
                    match iface.bulk_out(ep, blit_buf).await.into_result() {
                        Ok(_) => tracing::info!("Splash screen sent successfully"),
                        Err(e) => tracing::error!("Splash screen bulk_out failed: {}", e),
                    }
                });
            }
        }
    }

    // Main I/O loop. Fully event-driven: the thread parks here and only wakes when
    // either a real USB packet arrives on the interrupt endpoint, or the app sends
    // a command (LED/screen update). No polling, no timers, no busy-waiting.
    let mut event_buf = Vec::with_capacity(64);
    let mut running = true;

    enum Woken<T> {
        Read(T),
        Cmd(DeviceCmd),
        CmdChannelClosed,
    }

    block_on(async {
        // Queue initial interrupt read
        let mut pending_read = control_iface.interrupt_in(input_ep, RequestBuffer::new(1024));

        while running {
            let woken = futures_lite::future::or(
                async { Woken::Read((&mut pending_read).await) },
                async {
                    match cmd_rx.recv().await {
                        Ok(cmd) => Woken::Cmd(cmd),
                        Err(_) => Woken::CmdChannelClosed,
                    }
                },
            )
            .await;

            match woken {
                Woken::CmdChannelClosed => {
                    running = false;
                }
                Woken::Cmd(first_cmd) => {
                    // Apply this command, then drain any others already queued so a
                    // burst of updates (e.g. all 8 LEDs) gets batched into one flush.
                    let mut cmd = Some(first_cmd);
                    while let Some(c) = cmd.take() {
                        match c {
                            DeviceCmd::Disconnect => {
                                running = false;
                            }
                            DeviceCmd::SetLed { name, value } => {
                                for lb in &mut led_builders {
                                    if lb.set(&name, value) {
                                        break;
                                    }
                                }
                            }
                            DeviceCmd::SetLedInGroup { group, name, value } => {
                                for lb in &mut led_builders {
                                    if lb.group_id() == group {
                                        lb.set(&name, value);
                                        break;
                                    }
                                }
                            }
                            DeviceCmd::SetLedStrip { name, values } => {
                                for lb in &mut led_builders {
                                    if lb.set_strip(&name, &values) {
                                        break;
                                    }
                                }
                            }
                            DeviceCmd::SetLedStripInGroup { group, name, values } => {
                                for lb in &mut led_builders {
                                    if lb.group_id() == group {
                                        lb.set_strip(&name, &values);
                                        break;
                                    }
                                }
                            }
                            DeviceCmd::SubmitScreen { screen, pixels, format } => {
                                if let Some(sm) = screen_managers.get_mut(&screen) {
                                    let screen_desc =
                                        descriptor.screens.iter().find(|s| s.name == screen);
                                    if let Some(screen_desc) = screen_desc {
                                        if let Some(blit_data) =
                                            sm.submit(&pixels, format, screen_desc)
                                        {
                                            let screen_iface_id = &screen_desc.interface;
                                            if let Some(iface) = interfaces.get(screen_iface_id) {
                                                let ep = descriptor
                                                    .interface_by_id(screen_iface_id)
                                                    .and_then(|i| i.endpoints.out.as_ref())
                                                    .map(|ep| ep.address.0 as u8)
                                                    .unwrap_or(0x02);
                                                let _ = iface.bulk_out(ep, blit_data).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if running {
                            match cmd_rx.try_recv() {
                                Ok(next) => cmd = Some(next),
                                Err(_) => cmd = None,
                            }
                        }
                    }

                    if !running {
                        break;
                    }

                    // Flush any LEDs that were just changed
                    for lb in &mut led_builders {
                        if let Some(wire_buf) = lb.flush() {
                            let ep = lb.endpoint();
                            let _ = control_iface.interrupt_out(ep, wire_buf).await;
                        }
                    }
                }
                Woken::Read(completion) => {
                    match completion.into_result() {
                        Ok(data) => {
                            // Parse the packet
                            event_buf.clear();
                            if !hook.on_packet(device_id, &data, &mut event_buf) {
                                parser.parse(&data, &mut event_buf);
                            }
                            // Emit events
                            for event in event_buf.drain(..) {
                                if event_tx.send(event).is_err() {
                                    running = false;
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("USB read error: {}", e);
                            event_tx.send(Event::DeviceDisconnected { id: device_id }).ok();
                            running = false;
                            break;
                        }
                    }

                    if running {
                        // Queue next read
                        pending_read =
                            control_iface.interrupt_in(input_ep, RequestBuffer::new(1024));
                    }
                }
            }
        }
    });

    // Cleanup: clear LEDs and screen
    for lb in &mut led_builders {
        lb.clear();
        if let Some(wire_buf) = lb.flush() {
            let ep = lb.endpoint();
            block_on(async {
                let _ = control_iface.interrupt_out(ep, wire_buf).await;
            });
        }
    }

    tracing::info!("Device thread exiting for {:?}", device_id);
}

use std::collections::HashMap;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, Instant};

use async_io::Timer;
use crossbeam_channel::Sender;
use futures_lite::future::block_on;
use nusb::transfer::{Buffer, Bulk, Completion, ControlOut, ControlType, In, Interrupt, Out, Recipient};
use nusb::MaybeFuture;

use crate::core::descriptor::*;
use crate::core::event::{DeviceId, Event};
use crate::core::led::LedValue;
use crate::device::hooks::{NoopHook, PacketHook};
use crate::device::led_builder::LedBuilder;
use crate::device::parser::PacketParser;
use crate::screen::ScreenManager;

/// Modest real-time scheduling priority for USB input threads: below typical
/// pro-audio engine thread priority (often 60-90), above normal (0) — enough
/// to preempt desktop scheduling noise without contending with the consuming
/// app's audio engine for CPU time.
#[cfg(target_os = "linux")]
const RT_PRIORITY: i32 = 10;

/// Best-effort: elevate the given thread (by kernel TID) to SCHED_FIFO at
/// `priority`. Never fails hard — callers log and continue at normal
/// priority on error, since real-time scheduling is a latency nice-to-have,
/// not a requirement (not every system grants it).
#[cfg(target_os = "linux")]
fn try_set_realtime_priority(tid: libc::pid_t, priority: i32) -> std::io::Result<()> {
    let param = libc::sched_param { sched_priority: priority };
    let rc = unsafe { libc::sched_setscheduler(tid, libc::SCHED_FIFO, &param) };
    if rc != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Kernel thread IDs of every thread currently in this process, read from
/// /proc/self/task. Used to identify nusb's lazily-spawned internal USB
/// event-reaper thread, which nusb's public API gives no handle to.
#[cfg(target_os = "linux")]
fn task_tids() -> std::collections::HashSet<i32> {
    std::fs::read_dir("/proc/self/task")
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect()
}

/// Guards the one-time (process-wide, not per-device) attempt to identify
/// and elevate nusb's internal USB event thread. nusb (0.1.x) spawns this
/// thread lazily via plain `std::thread::spawn`, once, the first time any
/// device is opened (see nusb's platform/linux_usbfs/events.rs) — it
/// notifies completions for every USB transfer on every device in the
/// process, so it matters for input latency just as much as our own read
/// thread does. This relies on that exact lazy-spawn-on-first-open
/// behavior; if a future nusb upgrade changes it, this becomes a silent
/// no-op (logged at debug level) rather than breaking anything.
#[cfg(target_os = "linux")]
static NUSB_EVENT_THREAD_CHECK: std::sync::Once = std::sync::Once::new();

/// Commands sent from the main thread to a device thread.
pub enum DeviceCmd {
    SetLed { name: String, value: LedValue },
    SetLedInGroup { group: String, name: String, value: LedValue },
    SetLedStrip { name: String, values: Vec<u8> },
    SetLedStripInGroup { group: String, name: String, values: Vec<u8> },
    SubmitScreen { screen: String, pixels: Vec<u8>, format: PixelFormat },
    SubmitDualScreen {
        left_screen: String,
        right_screen: String,
        pixels: Vec<u8>,
        format: PixelFormat,
    },
    WriteInterface { interface: String, data: Vec<u8> },
    Disconnect,
}

/// Manages a single connected device's I/O thread.
pub struct DeviceHandle {
    pub device_id: DeviceId,
    pub descriptor: Arc<DeviceDescriptor>,
    cmd_tx: async_channel::Sender<DeviceCmd>,
    led_tx: Option<async_channel::Sender<DeviceCmd>>,
    screen_tx: Option<async_channel::Sender<DeviceCmd>>,
    thread: Option<thread::JoinHandle<()>>,
    screen_thread: Option<thread::JoinHandle<()>>,
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
        let has_screens = !descriptor.screens.is_empty();
        let (screen_tx, screen_rx) = if has_screens {
            let (tx, rx) = async_channel::bounded(8);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        // Dedicated channel for LED commands — processed on its own thread so
        // interrupt OUT transfers never block the interrupt IN read loop.
        let has_leds = !descriptor.leds.is_empty();
        let (led_tx, led_rx) = if has_leds {
            let (tx, rx) = async_channel::bounded(256);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        #[cfg(target_os = "linux")]
        let tids_before = {
            let mut captured = None;
            NUSB_EVENT_THREAD_CHECK.call_once(|| captured = Some(task_tids()));
            captured
        };

        let usb_device = usb_info.open().wait()?;

        #[cfg(target_os = "linux")]
        if let Some(before) = tids_before {
            // Give nusb's lazily-spawned event thread a moment to actually start.
            std::thread::sleep(std::time::Duration::from_millis(20));
            let new_tids: Vec<i32> = task_tids().difference(&before).copied().collect();
            match new_tids.as_slice() {
                [tid] => match try_set_realtime_priority(*tid, RT_PRIORITY) {
                    Ok(()) => tracing::info!(
                        "Elevated nusb's internal USB event thread (tid={tid}) to real-time priority"
                    ),
                    Err(e) => tracing::warn!(
                        "Could not elevate nusb's event thread to real-time priority: {e}"
                    ),
                },
                other => tracing::debug!(
                    "Could not uniquely identify nusb's event thread ({} new thread(s) found); skipping priority elevation for it",
                    other.len()
                ),
            }
        }

        let desc = descriptor.clone();
        let id = device_id;

        let thread = thread::Builder::new()
            .name(format!("encdr-{}", descriptor.name.replace(' ', "-").to_lowercase()))
            .spawn(move || {
                run_device(id, desc, usb_device, names, cmd_rx, led_rx, screen_rx, event_tx, gpu);
            })
            .map_err(|e| crate::core::error::EncdrError::Io(e))?;

        Ok(Self {
            device_id,
            descriptor,
            cmd_tx,
            led_tx,
            screen_tx,
            thread: Some(thread),
            screen_thread: None,
        })
    }

    pub fn send(&self, cmd: DeviceCmd) {
        match &cmd {
            DeviceCmd::SubmitScreen { .. } | DeviceCmd::SubmitDualScreen { .. } => {
                if let Some(ref tx) = self.screen_tx {
                    tx.try_send(cmd).ok();
                }
            }
            DeviceCmd::SetLed { .. }
            | DeviceCmd::SetLedInGroup { .. }
            | DeviceCmd::SetLedStrip { .. }
            | DeviceCmd::SetLedStripInGroup { .. } => {
                if let Some(ref tx) = self.led_tx {
                    tx.try_send(cmd).ok();
                }
            }
            DeviceCmd::WriteInterface { .. } => {
                self.cmd_tx.try_send(cmd).ok();
            }
            DeviceCmd::Disconnect => {
                self.cmd_tx.try_send(DeviceCmd::Disconnect).ok();
                if let Some(ref tx) = self.led_tx {
                    tx.try_send(DeviceCmd::Disconnect).ok();
                }
                if let Some(ref tx) = self.screen_tx {
                    tx.try_send(DeviceCmd::Disconnect).ok();
                }
            }
        }
    }

    pub fn disconnect(mut self) {
        self.send(DeviceCmd::Disconnect);
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
        if let Some(thread) = self.screen_thread.take() {
            thread.join().ok();
        }
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        self.send(DeviceCmd::Disconnect);
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
        if let Some(thread) = self.screen_thread.take() {
            thread.join().ok();
        }
    }
}

/// Helper to determine the actual USB interface number for an interface descriptor.
/// If the interface descriptor's configured `number` lacks the required endpoints on
/// the physical device, this searches the device's USB configurations for an interface
/// that actually contains those endpoints.
fn resolve_interface_number(
    usb_device: &nusb::Device,
    iface_desc: &crate::core::descriptor::InterfaceDesc,
) -> u8 {
    let mut req_eps = Vec::new();
    if let Some(ref ep) = iface_desc.endpoints.ep_in {
        req_eps.push(ep.address.0 as u8);
    }
    if let Some(ref ep) = iface_desc.endpoints.out {
        req_eps.push(ep.address.0 as u8);
    }

    if req_eps.is_empty() {
        return iface_desc.number;
    }

    // Build map of interface_number -> list of endpoint addresses
    let mut iface_eps: HashMap<u8, Vec<u8>> = HashMap::new();
    for config in usb_device.configurations() {
        for intf in config.interfaces() {
            let num = intf.interface_number();
            let entry = iface_eps.entry(num).or_default();
            for alt in intf.alt_settings() {
                for ep in alt.endpoints() {
                    let addr = ep.address();
                    if !entry.contains(&addr) {
                        entry.push(addr);
                    }
                }
            }
        }
    }

    // 1. If configured number already has all required endpoints, keep it
    if let Some(eps) = iface_eps.get(&iface_desc.number) {
        if req_eps.iter().all(|req| eps.contains(req)) {
            return iface_desc.number;
        }
    }

    // 2. Search for an interface that contains all required endpoints
    for (&num, eps) in &iface_eps {
        if req_eps.iter().all(|req| eps.contains(req)) {
            tracing::info!(
                "Auto-resolved interface '{}' to USB interface {} (descriptor configured {}, required endpoints: {:02x?})",
                iface_desc.id, num, iface_desc.number, req_eps
            );
            return num;
        }
    }

    // 3. Fallback: search for an interface containing any of the required endpoints
    for (&num, eps) in &iface_eps {
        if req_eps.iter().any(|req| eps.contains(req)) {
            tracing::info!(
                "Partially resolved interface '{}' to USB interface {} (descriptor configured {}, required endpoints: {:02x?})",
                iface_desc.id, num, iface_desc.number, req_eps
            );
            return num;
        }
    }

    // 4. Default to descriptor's configured number
    iface_desc.number
}

/// Main device I/O loop. Runs on a dedicated thread.
fn run_device(
    device_id: DeviceId,
    descriptor: Arc<DeviceDescriptor>,
    usb_device: nusb::Device,
    names: HashMap<String, &'static str>,
    cmd_rx: async_channel::Receiver<DeviceCmd>,
    led_rx: Option<async_channel::Receiver<DeviceCmd>>,
    screen_rx: Option<async_channel::Receiver<DeviceCmd>>,
    event_tx: Sender<Event>,
    gpu: Option<Arc<crate::screen::GpuContext>>,
) {
    #[cfg(target_os = "linux")]
    {
        let tid = unsafe { libc::gettid() };
        match try_set_realtime_priority(tid, RT_PRIORITY) {
            Ok(()) => tracing::info!(
                "Device I/O thread elevated to SCHED_FIFO priority {RT_PRIORITY}"
            ),
            Err(e) => tracing::warn!(
                "Could not elevate device I/O thread to real-time priority ({e}); continuing at normal priority — input latency may be higher on this system"
            ),
        }
    }

    // Claim interfaces
    let mut claimed_by_num: HashMap<u8, nusb::Interface> = HashMap::new();
    let mut interfaces: HashMap<String, nusb::Interface> = HashMap::new();

    for iface_desc in &descriptor.interfaces {
        let target_num = resolve_interface_number(&usb_device, iface_desc);
        if !claimed_by_num.contains_key(&target_num) {
            match usb_device.detach_and_claim_interface(target_num).wait() {
                Ok(iface) => {
                    tracing::info!(
                        "Claimed interface {} ('{}')",
                        target_num,
                        iface_desc.id
                    );
                    // Several logical interfaces may share one USB interface
                    // number; any of them may carry the alternate setting.
                    let alt = descriptor
                        .interfaces
                        .iter()
                        .find(|i| i.id == iface_desc.id || i.number == target_num)
                        .and_then(|i| i.alt_setting);
                    if let Some(alt) = alt {
                        if let Err(e) = iface.set_alt_setting(alt).wait() {
                            tracing::error!(
                                "Failed to select alt setting {} on interface {}: {}",
                                alt,
                                target_num,
                                e
                            );
                            return;
                        }
                    }
                    claimed_by_num.insert(target_num, iface);
                }
                Err(e) => {
                    crate::usb::service_detector::diagnose_claim_failure(
                        target_num,
                        &iface_desc.id,
                        &e,
                    );
                    event_tx.send(Event::DeviceDisconnected { id: device_id }).ok();
                    return;
                }
            }
        }
        if let Some(iface) = claimed_by_num.get(&target_num) {
            interfaces.insert(iface_desc.id.clone(), iface.clone());
        }
    }

    // Determine the control interface for input reading
    let control_iface_id = descriptor
        .input_packets
        .first()
        .map(|p| p.interface.clone())
        .unwrap_or_else(|| "control".to_string());

    let Some(control_iface) = interfaces.get(&control_iface_id).cloned() else {
        tracing::error!("Control interface '{}' not found", control_iface_id);
        event_tx.send(Event::DeviceDisconnected { id: device_id }).ok();
        return;
    };

    // Open one IN endpoint per interface referenced by an input packet and
    // queue reads on all of them before anything else touches the device.
    // Pipeline reads: keep multiple reads pending with the kernel at all
    // times, rather than submitting one and waiting for it to complete
    // before submitting the next, so there's never a window with no read in
    // flight.
    const READ_QUEUE_DEPTH: usize = 8;
    let mut input_sources: Vec<InputSource> = Vec::new();
    for packet in &descriptor.input_packets {
        if input_sources.iter().any(|s| s.interface == packet.interface) {
            continue;
        }
        let Some(iface) = interfaces.get(&packet.interface) else {
            tracing::error!("Input interface '{}' not found", packet.interface);
            event_tx.send(Event::DeviceDisconnected { id: device_id }).ok();
            return;
        };
        let (address, transfer_type) = descriptor
            .interface_by_id(&packet.interface)
            .and_then(|i| i.endpoints.ep_in.as_ref())
            .map(|ep| (ep.address.0 as u8, ep.transfer_type))
            .unwrap_or((0x81, TransferType::Interrupt));
        let mut ep = match InEndpoint::open(iface, address, transfer_type) {
            Ok(ep) => ep,
            Err(e) => {
                let available_eps: Vec<String> = iface
                    .descriptor()
                    .map(|d| {
                        d.endpoints()
                            .map(|ep| format!("0x{:02x} ({:?})", ep.address(), ep.transfer_type()))
                            .collect()
                    })
                    .unwrap_or_default();

                let all_ifaces: Vec<String> = usb_device
                    .configurations()
                    .flat_map(|c| c.interfaces())
                    .map(|intf| {
                        let eps: Vec<String> = intf
                            .alt_settings()
                            .flat_map(|alt| {
                                alt.endpoints().map(|ep| {
                                    format!("0x{:02x} ({:?})", ep.address(), ep.transfer_type())
                                })
                            })
                            .collect();
                        format!("iface #{}: [{}]", intf.interface_number(), eps.join(", "))
                    })
                    .collect();

                tracing::error!(
                    "Failed to open input endpoint 0x{:02x} on interface {} ('{}'): {}. \
                     Endpoints available on this interface: [{}]. \
                     All interfaces on device: {}",
                    address,
                    iface.interface_number(),
                    packet.interface,
                    e,
                    available_eps.join(", "),
                    all_ifaces.join("; ")
                );
                event_tx.send(Event::DeviceDisconnected { id: device_id }).ok();
                return;
            }
        };

        // A USB transfer only completes on a short packet or when its buffer is
        // full. Sizing the buffer larger than the biggest report makes full-size
        // reports (e.g. the Mk3's 64-byte pad report on a 64-byte endpoint)
        // accumulate in the kernel until the buffer fills or an unrelated short
        // report happens to flush them — seconds of latency, and reports glued
        // together that the parser can't dispatch. Size the buffer to the
        // largest report, rounded up to a whole number of packets, so every
        // report completes its own transfer.
        let mps = ep.max_packet_size().max(1);
        let max_report = descriptor
            .input_packets
            .iter()
            .filter(|p| p.interface == packet.interface)
            .map(|p| p.size)
            .max()
            .unwrap_or(mps);
        let read_buf_size = max_report.div_ceil(mps) * mps;
        tracing::debug!(
            "Input ep 0x{:02x} ('{}'): max packet {} bytes, read buffer {} bytes",
            address, packet.interface, mps, read_buf_size
        );

        while ep.pending() < READ_QUEUE_DEPTH {
            let buf = ep.allocate(read_buf_size);
            ep.submit(buf);
        }
        input_sources.push(InputSource { interface: packet.interface.clone(), ep });
    }

    // One-shot init writes. Reads are already queued, so a device that holds
    // commands until its replies are read (Maschine Mk1) cannot stall here.
    // This runs before the LED and screen threads open their endpoints, since
    // nusb grants each endpoint to one owner at a time.
    for write in &descriptor.quirks.init_writes {
        let target = interfaces.get(&write.interface).zip(
            descriptor
                .interface_by_id(&write.interface)
                .and_then(|i| i.endpoints.out.as_ref()),
        );
        let Some((iface, ep_desc)) = target else {
            tracing::error!("Init write interface '{}' has no OUT endpoint", write.interface);
            continue;
        };
        let data: Vec<u8> = write.data.iter().map(|b| b.0 as u8).collect();
        let address = ep_desc.address.0 as u8;
        let result = match ep_desc.transfer_type {
            TransferType::Bulk => iface
                .endpoint::<Bulk, Out>(address)
                .map(|mut ep| ep.transfer_blocking(data.clone().into(), INIT_WRITE_TIMEOUT).status),
            _ => iface
                .endpoint::<Interrupt, Out>(address)
                .map(|mut ep| ep.transfer_blocking(data.clone().into(), INIT_WRITE_TIMEOUT).status),
        };
        match result {
            Ok(Ok(())) => tracing::debug!("Init write to ep 0x{:02x}: {:02x?}", address, data),
            Ok(Err(e)) => tracing::warn!("Init write to ep 0x{:02x} failed: {}", address, e),
            Err(e) => tracing::warn!("Could not open ep 0x{:02x} for init write: {}", address, e),
        }
    }

    // Build LED controllers (one per LED group)
    let led_builders: Vec<LedBuilder> = descriptor
        .leds
        .iter()
        .filter_map(|led_desc| {
            let iface = descriptor.interface_by_id(&led_desc.interface)?;
            Some(LedBuilder::new(led_desc, iface))
        })
        .collect();

    // If device has screens, delegate them entirely to a dedicated screen thread
    // so that screen bulk transfers never stall the interrupt input loop.
    let screen_thread = if !descriptor.screens.is_empty() && screen_rx.is_some() {
        let screen_rx = screen_rx.unwrap();
        let screen_desc = descriptor.clone();
        let mut screen_ifaces = HashMap::new();
        for s in &descriptor.screens {
            if let Some(iface) = interfaces.get(&s.interface) {
                screen_ifaces.insert(s.interface.clone(), iface.clone());
            }
        }
        let mut screen_managers = HashMap::new();
        for s in &descriptor.screens {
            let sm = ScreenManager::new(s, gpu.clone());
            screen_managers.insert(s.name.clone(), sm);
        }

        thread::Builder::new()
            .name(format!("encdr-screen-{}", descriptor.name.replace(' ', "-").to_lowercase()))
            .spawn(move || {
                run_screens(screen_desc, screen_ifaces, screen_managers, screen_rx);
            })
            .ok()
    } else {
        None
    };

    // Spawn a dedicated LED thread so that interrupt OUT transfers never block the
    // interrupt IN read loop. The LED thread gets its own clone of control_iface
    // (nusb::Interface is Arc-based and supports concurrent transfers).
    let led_thread = if let Some(led_rx) = led_rx {
        let led_iface = control_iface.clone();
        let led_builders = led_builders;
        let feature_leds = descriptor.quirks.feature_report_leds.clone();
        thread::Builder::new()
            .name(format!("encdr-led-{}", descriptor.name.replace(' ', "-").to_lowercase()))
            .spawn(move || {
                run_leds(led_iface, led_builders, feature_leds, led_rx);
            })
            .ok()
    } else {
        None
    };

    // Build packet parser
    let mut parser = PacketParser::new(device_id, &descriptor, names);
    let mut hook: Box<dyn PacketHook> = Box::new(NoopHook);


    // Main I/O loop — READ-ONLY. This thread now exclusively handles:
    //   1. USB interrupt IN reads (highest priority)
    //   2. Periodic pad timeout sweeps
    //   3. Disconnect commands
    //
    // LED writes and screen blits run on their own threads and never contend
    // with reads. This guarantees pad events are processed with minimum latency.
    let mut event_buf = Vec::with_capacity(64);
    let mut running = true;

    const PAD_TIMEOUT_POLL_INTERVAL: Duration = Duration::from_millis(40);

    enum Woken {
        Read(usize, Completion),
        Cmd(DeviceCmd),
        CmdChannelClosed,
        TimeoutTick,
    }

    block_on(async {
        let mut last_timeout_sweep = Instant::now();
        // Index of the input source served most recently (round-robin cursor).
        let mut last_served = 0;

        while running {
            let remaining = PAD_TIMEOUT_POLL_INTERVAL.saturating_sub(last_timeout_sweep.elapsed());
            let woken = futures_lite::future::or(
                futures_lite::future::or(
                    std::future::poll_fn(|cx| {
                        // Start after the endpoint served last, so a busy
                        // stream (e.g. continuous pads) can't starve the others.
                        let count = input_sources.len();
                        for offset in 1..=count {
                            let idx = (last_served + offset) % count;
                            if let Poll::Ready(completion) = input_sources[idx].ep.poll_next_complete(cx) {
                                last_served = idx;
                                return Poll::Ready(Woken::Read(idx, completion));
                            }
                        }
                        Poll::Pending
                    }),
                    async {
                        match cmd_rx.recv().await {
                            Ok(cmd) => Woken::Cmd(cmd),
                            Err(_) => Woken::CmdChannelClosed,
                        }
                    },
                ),
                async {
                    Timer::after(remaining).await;
                    Woken::TimeoutTick
                },
            )
            .await;

            match woken {
                Woken::CmdChannelClosed => {
                    running = false;
                }
                Woken::TimeoutTick => {
                    // Timeout tick wakes up the loop so the sweep below executes
                }
                Woken::Cmd(cmd) => {
                    match cmd {
                        DeviceCmd::Disconnect => {
                            running = false;
                            break;
                        }
                        DeviceCmd::WriteInterface { interface, data } => {
                            let target = interfaces.get(&interface).zip(
                                descriptor
                                    .interface_by_id(&interface)
                                    .and_then(|i| i.endpoints.out.as_ref()),
                            );
                            if let Some((iface, ep_desc)) = target {
                                let address = ep_desc.address.0 as u8;
                                let len = data.len();
                                let timeout = Duration::from_millis(1000);
                                let res = match ep_desc.transfer_type {
                                    TransferType::Bulk => iface
                                        .endpoint::<Bulk, Out>(address)
                                        .map(|mut ep| ep.transfer_blocking(data.into(), timeout).status),
                                    _ => iface
                                        .endpoint::<Interrupt, Out>(address)
                                        .map(|mut ep| ep.transfer_blocking(data.into(), timeout).status),
                                };
                                match res {
                                    Ok(Ok(())) => tracing::trace!("Wrote {} bytes to interface '{}' (ep 0x{:02x})", len, interface, address),
                                    Ok(Err(e)) => tracing::warn!("Write to interface '{}' (ep 0x{:02x}) failed: {}", interface, address, e),
                                    Err(e) => tracing::warn!("Could not open ep 0x{:02x} on interface '{}': {}", address, interface, e),
                                }
                            } else {
                                tracing::warn!("WriteInterface: interface '{}' not found or has no OUT endpoint", interface);
                            }
                        }
                        _ => {}
                    }
                }
                Woken::Read(idx, completion) => {
                    let source = &mut input_sources[idx];
                    let buf = completion.buffer;
                    match completion.status {
                        Ok(()) => {
                            let data = &buf[..completion.actual_len];
                            tracing::trace!("USB RECV [len={}] header={:02x?}", data.len(), &data[0..data.len().min(8)]);

                            // Parse the packet
                            event_buf.clear();
                            if !hook.on_packet(device_id, data, &mut event_buf) {
                                parser.parse_from(&source.interface, data, &mut event_buf);
                            }

                            // Emit events
                            for event in event_buf.drain(..) {
                                if event_tx.send(event).is_err() {
                                    running = false;
                                    break;
                                }
                            }

                            if running {
                                // Resubmit immediately, recycling the zero-copy buffer,
                                // to keep the queue at constant depth so the kernel
                                // always has a read pending.
                                source.ep.submit(buf);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("USB read error: {}", e);
                            event_tx.send(Event::DeviceDisconnected { id: device_id }).ok();
                            running = false;
                            break;
                        }
                    }
                }
            }

            // Periodic pad timeout sweep — runs whenever PAD_TIMEOUT_POLL_INTERVAL
            // has elapsed, regardless of whether the loop was woken by Read, Cmd, or Timer.
            if running && last_timeout_sweep.elapsed() >= PAD_TIMEOUT_POLL_INTERVAL {
                last_timeout_sweep = Instant::now();
                event_buf.clear();
                parser.check_pad_timeouts(&mut event_buf);
                for event in event_buf.drain(..) {
                    if event_tx.send(event).is_err() {
                        running = false;
                        break;
                    }
                }
            }
        }
    });

    // Join worker threads to ensure clean exit transfers complete
    if let Some(th) = screen_thread {
        th.join().ok();
    }
    if let Some(th) = led_thread {
        th.join().ok();
    }

    tracing::info!("Device thread exiting for {:?}", device_id);
}

/// Upper bound on a single connect-time init write.
const INIT_WRITE_TIMEOUT: Duration = Duration::from_millis(500);

/// An input endpoint plus the descriptor interface id its packets belong to.
struct InputSource {
    interface: String,
    ep: InEndpoint,
}

/// IN endpoint of either transfer type (nusb encodes the type statically).
enum InEndpoint {
    Interrupt(nusb::Endpoint<Interrupt, In>),
    Bulk(nusb::Endpoint<Bulk, In>),
}

impl InEndpoint {
    fn open(iface: &nusb::Interface, address: u8, transfer_type: TransferType) -> Result<Self, nusb::Error> {
        Ok(match transfer_type {
            TransferType::Bulk => Self::Bulk(iface.endpoint::<Bulk, In>(address)?),
            _ => Self::Interrupt(iface.endpoint::<Interrupt, In>(address)?),
        })
    }

    fn max_packet_size(&self) -> usize {
        match self {
            Self::Interrupt(ep) => ep.max_packet_size(),
            Self::Bulk(ep) => ep.max_packet_size(),
        }
    }

    fn pending(&self) -> usize {
        match self {
            Self::Interrupt(ep) => ep.pending(),
            Self::Bulk(ep) => ep.pending(),
        }
    }

    fn allocate(&self, len: usize) -> Buffer {
        match self {
            Self::Interrupt(ep) => ep.allocate(len),
            Self::Bulk(ep) => ep.allocate(len),
        }
    }

    fn submit(&mut self, buf: Buffer) {
        match self {
            Self::Interrupt(ep) => ep.submit(buf),
            Self::Bulk(ep) => ep.submit(buf),
        }
    }

    fn poll_next_complete(&mut self, cx: &mut Context<'_>) -> Poll<Completion> {
        match self {
            Self::Interrupt(ep) => ep.poll_next_complete(cx),
            Self::Bulk(ep) => ep.poll_next_complete(cx),
        }
    }
}

/// OUT endpoint of either transfer type, used for screen and LED writes.
enum OutEndpoint {
    Bulk(nusb::Endpoint<Bulk, Out>),
    Interrupt(nusb::Endpoint<Interrupt, Out>),
}

impl OutEndpoint {
    fn open(iface: &nusb::Interface, address: u8, transfer_type: TransferType) -> Result<Self, nusb::Error> {
        Ok(match transfer_type {
            TransferType::Bulk => Self::Bulk(iface.endpoint::<Bulk, Out>(address)?),
            _ => Self::Interrupt(iface.endpoint::<Interrupt, Out>(address)?),
        })
    }

    /// Submit one transfer and wait for it to complete.
    async fn write(&mut self, data: Vec<u8>) -> Result<(), nusb::transfer::TransferError> {
        match self {
            Self::Bulk(ep) => {
                ep.submit(data.into());
                ep.next_complete().await.status
            }
            Self::Interrupt(ep) => {
                ep.submit(data.into());
                ep.next_complete().await.status
            }
        }
    }

    /// Send transfers one after another, stopping at the first error.
    async fn write_each(&mut self, transfers: Vec<Vec<u8>>) -> Result<(), nusb::transfer::TransferError> {
        for data in transfers {
            self.write(data).await?;
        }
        Ok(())
    }
}

/// Dedicated screen worker thread. Runs completely decoupled from the interrupt IN input loop
/// so that large bulk pixel transfers never delay button, pad, or encoder events.
fn run_screens(
    descriptor: Arc<DeviceDescriptor>,
    screen_ifaces: HashMap<String, nusb::Interface>,
    mut screen_managers: HashMap<String, ScreenManager>,
    screen_rx: async_channel::Receiver<DeviceCmd>,
) {
    use crate::screen::protocol;

    block_on(async {
        let mut endpoints: HashMap<(String, u8), OutEndpoint> = HashMap::new();
        let mut screen_to_ep: HashMap<String, (String, u8)> = HashMap::new();

        for s in &descriptor.screens {
            if let Some(iface) = screen_ifaces.get(&s.interface) {
                let ep_desc = descriptor
                    .interface_by_id(&s.interface)
                    .and_then(|i| i.endpoints.out.as_ref());
                let ep_addr = ep_desc.map(|ep| ep.address.0 as u8).unwrap_or(0x02);
                let transfer_type = ep_desc.map(|ep| ep.transfer_type).unwrap_or(TransferType::Bulk);
                let key = (s.interface.clone(), ep_addr);
                screen_to_ep.insert(s.name.clone(), key.clone());

                if !endpoints.contains_key(&key) {
                    match OutEndpoint::open(iface, ep_addr, transfer_type) {
                        Ok(ep) => {
                            endpoints.insert(key, ep);
                        }
                        Err(e) => tracing::error!(
                            "Failed to open screen OUT ep 0x{:02x} for '{}': {}",
                            ep_addr, s.name, e
                        ),
                    }
                }
            }
        }

        // Initialise screen controllers that need it, then send an initial
        // splash screen (black frame).
        'screens: for screen_desc in &descriptor.screens {
            let Some(ep) = screen_to_ep.get(&screen_desc.name).and_then(|key| endpoints.get_mut(key)) else {
                continue;
            };
            let steps = protocol::init_sequence(screen_desc);
            let step_count = steps.len();
            for (i, step) in steps.into_iter().enumerate() {
                if let Err(e) = ep.write(step.data).await {
                    tracing::error!(
                        "Screen init for '{}' failed at step {}/{}: {}; the screen will stay blank",
                        screen_desc.name,
                        i + 1,
                        step_count,
                        e
                    );
                    continue 'screens;
                }
                if !step.delay.is_zero() {
                    Timer::after(step.delay).await;
                }
            }

            let blank = vec![screen_desc.pixel_format.black_fill(); screen_desc.byte_size()];
            let blit_buf = protocol::build_full_blit(screen_desc, &blank);
            let transfers = protocol::frame_transfers(screen_desc, blit_buf);
            tracing::info!(
                "Sending splash screen: {} bytes in {} transfer(s) for '{}'",
                transfers.iter().map(Vec::len).sum::<usize>(),
                transfers.len(),
                screen_desc.name
            );
            match ep.write_each(transfers).await {
                Ok(()) => tracing::info!("Splash screen sent successfully for '{}'", screen_desc.name),
                Err(e) => tracing::error!("Splash screen for '{}' failed: {}", screen_desc.name, e),
            }
        }

        async fn process_and_send_screen(
            screen: &str,
            pixels: &[u8],
            format: PixelFormat,
            descriptor: &DeviceDescriptor,
            screen_managers: &mut HashMap<String, ScreenManager>,
            screen_to_ep: &HashMap<String, (String, u8)>,
            endpoints: &mut HashMap<(String, u8), OutEndpoint>,
        ) {
            let Some(sm) = screen_managers.get_mut(screen) else {
                tracing::warn!("ScreenManager for '{}' not found", screen);
                return;
            };
            let Some(screen_desc) = descriptor.screens.iter().find(|s| s.name == screen) else {
                tracing::warn!("screen_desc for '{}' not found in descriptor", screen);
                return;
            };
            let Some(blit_data) = sm.submit(pixels, format, screen_desc) else {
                return;
            };
            let Some(ep) = screen_to_ep.get(screen).and_then(|key| endpoints.get_mut(key)) else {
                tracing::warn!("Screen endpoint for '{}' not found", screen);
                return;
            };
            tracing::debug!("Sending {} bytes for screen '{}'...", blit_data.len(), screen);
            if let Err(e) = ep.write_each(protocol::frame_transfers(screen_desc, blit_data)).await {
                tracing::warn!("Screen transfer for '{}' failed: {}", screen, e);
            }
            tracing::debug!("Bulk out completed for screen '{}'", screen);
        }

        while let Ok(cmd) = screen_rx.recv().await {
            match cmd {
                DeviceCmd::Disconnect => break,
                DeviceCmd::SubmitScreen { screen, pixels, format } => {
                    process_and_send_screen(
                        &screen,
                        &pixels,
                        format,
                        &descriptor,
                        &mut screen_managers,
                        &screen_to_ep,
                        &mut endpoints,
                    )
                    .await;
                }
                DeviceCmd::SubmitDualScreen {
                    left_screen,
                    right_screen,
                    pixels,
                    format,
                } => {
                    let left_desc = descriptor.screens.iter().find(|s| s.name == left_screen);
                    let right_desc = descriptor.screens.iter().find(|s| s.name == right_screen);
                    let (Some(left_desc), Some(right_desc)) = (left_desc, right_desc) else {
                        tracing::warn!(
                            "SubmitDualScreen: could not find screens '{}' and/or '{}'",
                            left_screen,
                            right_screen
                        );
                        continue;
                    };
                    if left_desc.height != right_desc.height {
                        tracing::warn!(
                            "SubmitDualScreen: left height ({}) != right height ({})",
                            left_desc.height,
                            right_desc.height
                        );
                        continue;
                    }
                    let bpp = format.bytes_per_pixel();
                    let Some((left_pixels, right_pixels)) = crate::screen::split_horizontal(
                        &pixels,
                        left_desc.width as usize,
                        right_desc.width as usize,
                        left_desc.height as usize,
                        bpp,
                    ) else {
                        tracing::warn!(
                            "SubmitDualScreen: pixel buffer size mismatch (expected {} bytes, got {})",
                            (left_desc.width as usize + right_desc.width as usize)
                                * left_desc.height as usize
                                * bpp,
                            pixels.len()
                        );
                        continue;
                    };

                    process_and_send_screen(
                        &left_screen,
                        &left_pixels,
                        format,
                        &descriptor,
                        &mut screen_managers,
                        &screen_to_ep,
                        &mut endpoints,
                    )
                    .await;

                    process_and_send_screen(
                        &right_screen,
                        &right_pixels,
                        format,
                        &descriptor,
                        &mut screen_managers,
                        &screen_to_ep,
                        &mut endpoints,
                    )
                    .await;
                }
                _ => continue,
            }
        }

        // Clean exit: render screensaver image to color screens, blank monochrome screens
        for screen_desc in &descriptor.screens {
            let Some(ep) = screen_to_ep.get(&screen_desc.name).and_then(|key| endpoints.get_mut(key)) else {
                continue;
            };

            let native_pixels = if screen_desc.pixel_format == PixelFormat::Mono
                || screen_desc.pixel_format == PixelFormat::St7529Gray5
            {
                vec![screen_desc.pixel_format.black_fill(); screen_desc.byte_size()]
            } else {
                let rgba = crate::screen::screensaver::render_screensaver_frame(
                    screen_desc.width,
                    screen_desc.height,
                );
                crate::screen::convert_format(
                    &rgba,
                    PixelFormat::Rgba8888,
                    screen_desc.pixel_format,
                    screen_desc.width,
                    screen_desc.height,
                )
            };

            let blit_buf = protocol::build_full_blit(screen_desc, &native_pixels);
            let transfers = protocol::frame_transfers(screen_desc, blit_buf);
            tracing::info!(
                "Clean exit: sending screensaver to '{}' ({} bytes)",
                screen_desc.name,
                transfers.iter().map(Vec::len).sum::<usize>()
            );
            if let Err(e) = ep.write_each(transfers).await {
                tracing::warn!("Screensaver transfer for '{}' failed on clean exit: {}", screen_desc.name, e);
            }
        }
    });
}

/// Dedicated LED writer thread. Receives LED commands on its own channel, batches
/// them, and flushes via interrupt OUT on a cloned interface handle. Runs completely
/// decoupled from the interrupt IN read loop so USB output transfers never stall
/// input processing.
fn run_leds(
    iface: nusb::Interface,
    mut led_builders: Vec<LedBuilder>,
    feature_report_leds: Option<FeatureReportLedsQuirkDesc>,
    led_rx: async_channel::Receiver<DeviceCmd>,
) {
    block_on(async {
        let mut led_endpoints: HashMap<u8, OutEndpoint> = HashMap::new();
        for lb in &led_builders {
            let ep = lb.endpoint();
            if !led_endpoints.contains_key(&ep) {
                match OutEndpoint::open(&iface, ep, lb.transfer_type()) {
                    Ok(endpoint) => {
                        led_endpoints.insert(ep, endpoint);
                    }
                    Err(e) => tracing::error!("Failed to open LED OUT ep 0x{:02x}: {}", ep, e),
                }
            }
        }

        // Push descriptor default LED values (e.g. a display backlight).
        for lb in &mut led_builders {
            if let Some(wire_buf) = lb.flush() {
                if let Some(endpoint) = led_endpoints.get_mut(&lb.endpoint()) {
                    let _ = endpoint.write(wire_buf).await;
                }
            }
        }

        // Tracks cumulative mask and dirty state per feature subcommand (e.g. 0x26 -> (mask, dirty))
        let mut feature_cmd_masks: HashMap<u8, (u8, bool)> = HashMap::new();

        while let Ok(first_cmd) = led_rx.recv().await {
            // Apply this command, then drain any others already queued so a
            // burst of updates (e.g. all 16 pad LEDs) gets batched into one flush.
            let mut cmd = Some(first_cmd);
            let mut disconnected = false;
            while let Some(c) = cmd.take() {
                match c {
                    DeviceCmd::Disconnect => {
                        disconnected = true;
                        break;
                    }
                    DeviceCmd::SetLed { name, value } => {
                        let mut handled = false;
                        if let Some(ref quirk) = feature_report_leds {
                            if let Some(item) = quirk.items.get(&name) {
                                let cmd_byte = item.command.0 as u8;
                                let mask_byte = item.mask.0 as u8;
                                let entry = feature_cmd_masks.entry(cmd_byte).or_insert((0, false));
                                let old_mask = entry.0;
                                let is_on = match value {
                                    LedValue::Off => false,
                                    LedValue::Dim => true,
                                    LedValue::Bright => true,
                                    LedValue::Single(b) => b > 0,
                                    LedValue::Rgb { r, g, b } => (r | g | b) > 0,
                                };
                                if is_on {
                                    entry.0 |= mask_byte;
                                } else {
                                    entry.0 &= !mask_byte;
                                }
                                if entry.0 != old_mask {
                                    entry.1 = true;
                                }
                                handled = true;
                            }
                        }
                        if !handled {
                            for lb in &mut led_builders {
                                if lb.set(&name, value) {
                                    break;
                                }
                            }
                        }
                    }
                    DeviceCmd::SetLedInGroup { group, name, value } => {
                        let mut handled = false;
                        if let Some(ref quirk) = feature_report_leds {
                            if let Some(item) = quirk.items.get(&name) {
                                let cmd_byte = item.command.0 as u8;
                                let mask_byte = item.mask.0 as u8;
                                let entry = feature_cmd_masks.entry(cmd_byte).or_insert((0, false));
                                let old_mask = entry.0;
                                let is_on = match value {
                                    LedValue::Off => false,
                                    LedValue::Dim => true,
                                    LedValue::Bright => true,
                                    LedValue::Single(b) => b > 0,
                                    LedValue::Rgb { r, g, b } => (r | g | b) > 0,
                                };
                                if is_on {
                                    entry.0 |= mask_byte;
                                } else {
                                    entry.0 &= !mask_byte;
                                }
                                if entry.0 != old_mask {
                                    entry.1 = true;
                                }
                                handled = true;
                            }
                        }
                        if !handled {
                            for lb in &mut led_builders {
                                if lb.group_id() == group {
                                    lb.set(&name, value);
                                    break;
                                }
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
                    _ => {}
                }

                match led_rx.try_recv() {
                    Ok(next) => cmd = Some(next),
                    Err(_) => cmd = None,
                }
            }

            if disconnected {
                break;
            }

            // Flush any LED groups that were dirtied by this batch
            for lb in &mut led_builders {
                if let Some(wire_buf) = lb.flush() {
                    let ep = lb.endpoint();
                    let group = lb.group_id().to_string();
                    let len = wire_buf.len();
                    if let Some(endpoint) = led_endpoints.get_mut(&ep) {
                        if let Err(e) = endpoint.write(wire_buf).await {
                            eprintln!("[LED-USB-ERR] Failed to flush {} bytes to group '{}' ep 0x{:02x}: {}", len, group, ep, e);
                        }
                    }
                }
            }

            // Flush feature report LEDs via EP0 Control Transfer (SET_REPORT)
            if let Some(ref quirk) = feature_report_leds {
                for (&cmd_byte, entry) in &mut feature_cmd_masks {
                    if entry.1 {
                        entry.1 = false;
                        let mut buf = vec![0u8; quirk.payload_length];
                        if !buf.is_empty() {
                            buf[0] = quirk.report_id.0 as u8;
                        }
                        if buf.len() > 1 {
                            buf[1] = cmd_byte;
                        }
                        if buf.len() > 2 {
                            buf[2] = entry.0;
                        }
                        let control = ControlOut {
                            control_type: ControlType::Class,
                            recipient: Recipient::Interface,
                            request: 0x09, // SET_REPORT
                            value: (0x03 << 8) | (quirk.report_id.0 as u16),
                            index: quirk.interface as u16,
                            data: &buf,
                        };
                        let res = iface.control_out(control, Duration::from_millis(100)).await;
                        if let Err(e) = res {
                            eprintln!(
                                "[LED-CTRL-ERR] Failed to flush feature LED report 0x{:02x} cmd 0x{:02x}: {}",
                                quirk.report_id.0, cmd_byte, e
                            );
                        }
                    }
                }
            }
        }

        // Cleanup: clear all standard LEDs on shutdown
        for lb in &mut led_builders {
            lb.clear();
            if let Some(wire_buf) = lb.flush() {
                let ep = lb.endpoint();
                if let Some(endpoint) = led_endpoints.get_mut(&ep) {
                    let _ = endpoint.write(wire_buf).await;
                }
            }
        }

        // Cleanup: clear feature report LEDs on shutdown
        if let Some(ref quirk) = feature_report_leds {
            let mut all_cmd_bytes: std::collections::HashSet<u8> = feature_cmd_masks.keys().copied().collect();
            for item in quirk.items.values() {
                all_cmd_bytes.insert(item.command.0 as u8);
            }
            for cmd_byte in all_cmd_bytes {
                let mut buf = vec![0u8; quirk.payload_length];
                if !buf.is_empty() {
                    buf[0] = quirk.report_id.0 as u8;
                }
                if buf.len() > 1 {
                    buf[1] = cmd_byte;
                }
                let control = ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 0x09,
                    value: (0x03 << 8) | (quirk.report_id.0 as u16),
                    index: quirk.interface as u16,
                    data: &buf,
                };
                let _ = iface.control_out(control, Duration::from_millis(100)).await;
            }
        }
    });
}

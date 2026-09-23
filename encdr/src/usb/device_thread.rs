use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use async_io::Timer;
use crossbeam_channel::Sender;
use futures_lite::future::block_on;
use nusb::transfer::{Completion, ControlOut, ControlType, Recipient, RequestBuffer};

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

        let usb_device = usb_info.open()?;

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
            DeviceCmd::SubmitScreen { .. } => {
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

    let Some(control_iface) = interfaces.remove(&control_iface_id) else {
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

    // If device has screens, delegate them entirely to a dedicated screen thread
    // so that screen bulk transfers never stall the interrupt input loop.
    if !descriptor.screens.is_empty() && screen_rx.is_some() {
        let screen_rx = screen_rx.unwrap();
        let screen_desc = descriptor.clone();
        let mut screen_ifaces = HashMap::new();
        for s in &descriptor.screens {
            if let Some(iface) = interfaces.remove(&s.interface) {
                screen_ifaces.insert(s.interface.clone(), iface);
            }
        }
        let mut screen_managers = HashMap::new();
        for s in &descriptor.screens {
            let sm = ScreenManager::new(s, gpu.clone());
            screen_managers.insert(s.name.clone(), sm);
        }

        let _ = thread::Builder::new()
            .name(format!("encdr-screen-{}", descriptor.name.replace(' ', "-").to_lowercase()))
            .spawn(move || {
                run_screens(screen_desc, screen_ifaces, screen_managers, screen_rx);
            });
    }

    // Spawn a dedicated LED thread so that interrupt OUT transfers never block the
    // interrupt IN read loop. The LED thread gets its own clone of control_iface
    // (nusb::Interface is Arc-based and supports concurrent transfers).
    if let Some(led_rx) = led_rx {
        let led_iface = control_iface.clone();
        let led_builders = led_builders;
        let feature_leds = descriptor.quirks.feature_report_leds.clone();
        let _ = thread::Builder::new()
            .name(format!("encdr-led-{}", descriptor.name.replace(' ', "-").to_lowercase()))
            .spawn(move || {
                run_leds(led_iface, led_builders, feature_leds, led_rx);
            });
    }

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
        Read(Completion<Vec<u8>>),
        Cmd(DeviceCmd),
        CmdChannelClosed,
        TimeoutTick,
    }

    block_on(async {
        // Pipeline interrupt IN reads: keep multiple reads pending with the kernel
        // at all times, rather than submitting one and waiting for it to complete
        // before submitting the next. Without this, there's a window on every
        // packet where no read is in flight, during which NI's firmware — whose
        // pad protocol already batches up to 21 tuples per 64-byte set specifically
        // to tolerate a slow host — queues up backlog that then arrives all at once
        // in a burst, showing up as delayed input followed by several pads
        // appearing to trigger simultaneously. See nusb::transfer::Queue's own docs.
        const READ_QUEUE_DEPTH: usize = 4;
        const READ_BUF_SIZE: usize = 1024;

        let mut read_queue = control_iface.interrupt_in_queue(input_ep);
        while read_queue.pending() < READ_QUEUE_DEPTH {
            read_queue.submit(RequestBuffer::new(READ_BUF_SIZE));
        }

        let mut last_timeout_sweep = Instant::now();

        while running {
            let remaining = PAD_TIMEOUT_POLL_INTERVAL.saturating_sub(last_timeout_sweep.elapsed());
            let woken = futures_lite::future::or(
                futures_lite::future::or(
                    async { Woken::Read(read_queue.next_complete().await) },
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
                    // The only command still routed here is Disconnect.
                    // LED and Screen commands go to their dedicated threads.
                    if matches!(cmd, DeviceCmd::Disconnect) {
                        running = false;
                        break;
                    }
                }
                Woken::Read(completion) => {
                    match completion.status {
                        Ok(()) => {
                            let data = completion.data;
                            tracing::trace!("USB RECV [len={}] header={:02x?}", data.len(), &data[0..data.len().min(8)]);

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

                            if running {
                                // Resubmit immediately, reusing the buffer allocation,
                                // to keep the queue at constant depth so the kernel
                                // always has a read pending.
                                read_queue.submit(RequestBuffer::reuse(data, READ_BUF_SIZE));
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

    tracing::info!("Device thread exiting for {:?}", device_id);
}

/// Dedicated screen worker thread. Runs completely decoupled from the interrupt IN input loop
/// so that large bulk pixel transfers never delay button, pad, or encoder events.
fn run_screens(
    descriptor: Arc<DeviceDescriptor>,
    screen_ifaces: HashMap<String, nusb::Interface>,
    mut screen_managers: HashMap<String, ScreenManager>,
    screen_rx: async_channel::Receiver<DeviceCmd>,
) {
    block_on(async {
        // Send initial splash screen (blank frame)
        for (screen_name, _sm) in &screen_managers {
            if let Some(screen_desc) = descriptor.screens.iter().find(|s| s.name == *screen_name) {
                if let Some(iface) = screen_ifaces.get(&screen_desc.interface) {
                    let ep = descriptor
                        .interface_by_id(&screen_desc.interface)
                        .and_then(|i| i.endpoints.out.as_ref())
                        .map(|ep| ep.address.0 as u8)
                        .unwrap_or(0x02);

                    let blank = vec![0u8; screen_desc.byte_size()];
                    let blit_buf = crate::screen::protocol::build_full_blit(screen_desc, &blank);
                    tracing::info!(
                        "Sending splash screen: {} bytes to ep 0x{:02x} on interface '{}'",
                        blit_buf.len(),
                        ep,
                        screen_desc.interface
                    );
                    let _ = iface.bulk_out(ep, blit_buf).await;
                    tracing::info!("Splash screen sent successfully");
                }
            }
        }

        while let Ok(cmd) = screen_rx.recv().await {
            match cmd {
                DeviceCmd::Disconnect => break,
                DeviceCmd::SubmitScreen { screen, pixels, format } => {
                    if let Some(sm) = screen_managers.get_mut(&screen) {
                        if let Some(screen_desc) = descriptor.screens.iter().find(|s| s.name == screen) {
                            if let Some(blit_data) = sm.submit(&pixels, format, screen_desc) {
                                if let Some(iface) = screen_ifaces.get(&screen_desc.interface) {
                                    let ep = descriptor
                                        .interface_by_id(&screen_desc.interface)
                                        .and_then(|i| i.endpoints.out.as_ref())
                                        .map(|ep| ep.address.0 as u8)
                                        .unwrap_or(0x02);
                                    let _ = iface.bulk_out(ep, blit_data).await;
                                }
                            }
                        }
                    }
                }
                _ => {}
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
        // Tracks cumulative mask and dirty state per feature subcommand (e.g. 0x26 -> (mask, dirty))
        let mut feature_cmd_masks: HashMap<u8, (u8, bool)> = HashMap::new();

        while let Ok(first_cmd) = led_rx.recv().await {
            // Apply this command, then drain any others already queued so a
            // burst of updates (e.g. all 16 pad LEDs) gets batched into one flush.
            let mut cmd = Some(first_cmd);
            while let Some(c) = cmd.take() {
                match c {
                    DeviceCmd::Disconnect => return,
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

            // Flush any LED groups that were dirtied by this batch
            for lb in &mut led_builders {
                if let Some(wire_buf) = lb.flush() {
                    let ep = lb.endpoint();
                    let group = lb.group_id().to_string();
                    let len = wire_buf.len();
                    let res = iface.interrupt_out(ep, wire_buf).await.into_result();
                    if let Err(e) = res {
                        eprintln!("[LED-USB-ERR] Failed to flush {} bytes to group '{}' ep 0x{:02x}: {}", len, group, ep, e);
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
                        let res = iface.control_out(control).await.into_result();
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
                let _ = iface.interrupt_out(ep, wire_buf).await;
            }
        }

        // Cleanup: clear feature report LEDs on shutdown
        if let Some(ref quirk) = feature_report_leds {
            for (&cmd_byte, entry) in &mut feature_cmd_masks {
                if entry.0 != 0 {
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
                    let _ = iface.control_out(control).await;
                }
            }
        }
    });
}

//! Named-pipe client for the NI hardware service (Windows).

use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe, SetNamedPipeHandleState,
    WaitNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};

use super::protocol::{self, TRUE_WORD};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pipe_path(name: &str) -> Vec<u16> {
    wide(&format!(r"\\.\pipe\{name}"))
}

/// Client end of a pipe the service created.
struct ClientPipe(HANDLE);

// SAFETY: a pipe HANDLE can be used from any thread; access is serialised by
// the callers (a `Mutex` around the only shared instance).
unsafe impl Send for ClientPipe {}
unsafe impl Sync for ClientPipe {}

impl Drop for ClientPipe {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl ClientPipe {
    fn open(name: &str, attempts: u32) -> Result<Self, String> {
        let path = pipe_path(name);
        let mut last = 0;
        for attempt in 0..attempts.max(1) {
            let h = unsafe {
                CreateFileW(
                    path.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if h != INVALID_HANDLE_VALUE {
                let mode = PIPE_READMODE_MESSAGE;
                if unsafe { SetNamedPipeHandleState(h, &mode, std::ptr::null(), std::ptr::null()) } == 0 {
                    tracing::debug!("SetNamedPipeHandleState failed for '{name}' ({})", unsafe { GetLastError() });
                }
                return Ok(Self(h));
            }
            last = unsafe { GetLastError() };
            if last == ERROR_PIPE_BUSY {
                unsafe { WaitNamedPipeW(path.as_ptr(), 500) };
            } else if attempt + 1 < attempts {
                thread::sleep(Duration::from_millis(200));
            }
        }
        Err(format!(r"could not open \\.\pipe\{name} (Windows error {last})"))
    }

    fn write(&self, data: &[u8]) -> Result<(), String> {
        let mut written: u32 = 0;
        let ok = unsafe { WriteFile(self.0, data.as_ptr().cast(), data.len() as u32, &mut written, std::ptr::null_mut()) };
        if ok == 0 {
            return Err(format!("writing to the NI service failed (Windows error {})", unsafe { GetLastError() }));
        }
        Ok(())
    }

    /// Wait up to `timeout` for a reply message. `Ok(None)` means none came.
    fn read_reply(&self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        let start = Instant::now();
        loop {
            let mut avail: u32 = 0;
            let peeked = unsafe {
                PeekNamedPipe(self.0, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut())
            };
            if peeked == 0 {
                return Err(format!("the NI service closed the pipe (Windows error {})", unsafe { GetLastError() }));
            }
            if avail > 0 {
                let mut buf = vec![0u8; avail.max(4096) as usize];
                let mut read: u32 = 0;
                let ok = unsafe { ReadFile(self.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut read, std::ptr::null_mut()) };
                if ok != 0 {
                    buf.truncate(read as usize);
                    return Ok(Some(buf));
                }
            }
            let waited = start.elapsed();
            if waited >= timeout {
                return Ok(None);
            }
            // Spin briefly for low latency, then back off.
            if waited < Duration::from_millis(20) {
                thread::yield_now();
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// Write and read the reply in one step; for the bootstrap pipe, where
    /// every request is answered.
    fn transact(&self, request: &[u8]) -> Result<Vec<u8>, String> {
        self.write(request)?;
        // Block in ReadFile rather than polling: the service closes the
        // bootstrap pipe right after replying, and a reply not yet read is
        // lost (Windows error 233) when it does.
        let mut buf = vec![0u8; 4096];
        let mut read: u32 = 0;
        let ok = unsafe { ReadFile(self.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut read, std::ptr::null_mut()) };
        if ok == 0 {
            return Err(format!("the NI service closed the pipe (Windows error {})", unsafe { GetLastError() }));
        }
        buf.truncate(read as usize);
        Ok(buf)
    }
}

/// Serve a notification pipe the service connects back to. Messages are
/// forwarded to `events`; each is acknowledged with an empty reply.
fn serve_notifications(name: String, events: Sender<Vec<u8>>) -> Result<(), String> {
    let path = pipe_path(&name);
    let h = unsafe {
        CreateNamedPipeW(
            path.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            65536,
            65536,
            5000,
            std::ptr::null(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(format!(r"could not create \\.\pipe\{name} (Windows error {})", unsafe { GetLastError() }));
    }
    let h = h as isize;
    thread::Builder::new()
        .name("encdr-ni-notify".to_string())
        .spawn(move || {
            let h = h as HANDLE;
            loop {
                let connected = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) };
                if connected == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
                    break;
                }
                loop {
                    let mut buf = vec![0u8; 65536];
                    let mut read: u32 = 0;
                    let ok = unsafe { ReadFile(h, buf.as_mut_ptr().cast(), buf.len() as u32, &mut read, std::ptr::null_mut()) };
                    if ok == 0 || read == 0 {
                        break;
                    }
                    buf.truncate(read as usize);
                    tracing::trace!(
                        "NI service notification on '{name}': {} bytes {:02x?}",
                        buf.len(),
                        &buf[..buf.len().min(128)]
                    );
                    let _ = events.send(buf);
                    let mut written: u32 = 0;
                    let acked = unsafe { WriteFile(h, [0u8; 1].as_ptr().cast(), 0, &mut written, std::ptr::null_mut()) };
                    if acked == 0 {
                        break;
                    }
                }
                unsafe { DisconnectNamedPipe(h) };
            }
            unsafe { CloseHandle(h) };
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Register with the service for the bootstrap step shared by device and
/// instance connects, returning the request pipe.
fn register(request: &[u8], events: &Sender<Vec<u8>>) -> Result<ClientPipe, String> {
    let bootstrap = ClientPipe::open(protocol::BOOTSTRAP_PIPE, 10)?;
    let reply = bootstrap.transact(request).map_err(|e| format!("connect request: {e}"))?;
    drop(bootstrap);
    let ports = protocol::parse_connect_reply(&reply)?;
    serve_notifications(ports.notification.clone(), events.clone())?;
    let request_pipe = ClientPipe::open(&ports.request, 10)?;
    request_pipe
        .write(&protocol::ack_notification_port(&ports.notification))
        .map_err(|e| format!("notification port ack: {e}"))?;
    request_pipe.read_reply(Duration::from_millis(500)).map_err(|e| format!("notification port ack reply: {e}"))?;
    Ok(request_pipe)
}

/// A registered client session for one physical device.
pub struct ServiceSession {
    /// Held open so the service keeps the device-level registration.
    _device: Mutex<ClientPipe>,
    instance: Mutex<ClientPipe>,
    control: async_channel::Receiver<bool>,
}

impl ServiceSession {
    /// Connect to the service and register for the unit with the given
    /// serial number (or the first one it reports when `serial` is `None` or
    /// not found).
    pub fn connect(product_id: u16, serial: Option<&str>, timeout: Duration) -> Result<Self, String> {
        if ClientPipe::open(protocol::BOOTSTRAP_PIPE, 1).is_err() {
            return Err("the NI hardware service is not running".to_string());
        }

        let (tx, rx) = channel::<Vec<u8>>();
        let device = register(&protocol::device_connect(product_id), &tx)?;
        device.write(&protocol::device_state_request()).map_err(|e| format!("device state request: {e}"))?;

        let serial = super::pick_serial(&rx, serial, timeout)?;
        tracing::info!("NI service reports device {serial}; registering for its screens");
        let control = super::watch_control(rx);

        let instance = register(&protocol::instance_connect(product_id, &serial), &tx)?;
        for (i, message) in protocol::instance_setup(product_id).into_iter().enumerate() {
            instance.write(&message).map_err(|e| format!("setup message {i}: {e}"))?;
            instance.read_reply(Duration::from_millis(40)).map_err(|e| format!("setup message {i} reply: {e}"))?;
        }

        Ok(Self { _device: Mutex::new(device), instance: Mutex::new(instance), control })
    }

    /// Changes in whether this client has the hardware: `true` when it became
    /// the active instance, `false` when another client took over.
    pub fn control_events(&self) -> async_channel::Receiver<bool> {
        self.control.clone()
    }

    /// Draw a screen command stream (the bytes a USB bulk transfer would
    /// carry) on display `display`, waiting for the service to accept it.
    pub fn send_display(&self, display: u8, width: u16, height: u16, commands: &[u8]) -> Result<(), String> {
        let message = protocol::display_message(display, width, height, commands);
        let pipe = self.instance.lock().map_err(|_| "NI service session poisoned".to_string())?;
        pipe.write(&message)?;
        match pipe.read_reply(protocol::DISPLAY_REPLY_TIMEOUT)? {
            Some(reply) if reply.get(..4).is_some_and(|w| w == TRUE_WORD.to_le_bytes()) => Ok(()),
            Some(reply) => Err(format!("{} (reply {:02x?})", super::REJECTED_FRAME,&reply[..reply.len().min(8)])),
            None => Err("the NI service did not acknowledge a frame in time".to_string()),
        }
    }
}

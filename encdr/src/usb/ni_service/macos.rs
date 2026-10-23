//! Message-port client for the NI hardware service (macOS).
//!
//! The service speaks the same messages as on Windows, but over
//! `CFMessagePort`s (Mach ports): each request is one message and its reply
//! comes back with it.
//!
//! The kernel owns the data handed to a port callback and reclaims it as soon
//! as the callback returns, so callbacks only copy it into an owned buffer and
//! queue it. Replies are copied the same way before they are released.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use core_foundation_sys::base::{kCFAllocatorDefault, Boolean, CFRelease};
use core_foundation_sys::data::{CFDataCreate, CFDataGetBytePtr, CFDataGetLength, CFDataRef};
use core_foundation_sys::messageport::{
    kCFMessagePortSuccess, CFMessagePortContext, CFMessagePortCreateLocal, CFMessagePortCreateRemote,
    CFMessagePortCreateRunLoopSource, CFMessagePortInvalidate, CFMessagePortRef, CFMessagePortSendRequest,
};
use core_foundation_sys::runloop::{
    kCFRunLoopDefaultMode, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRemoveSource, CFRunLoopRunInMode,
};
use core_foundation_sys::string::{kCFStringEncodingUTF8, CFStringCreateWithBytes, CFStringRef};

use super::protocol::{self, TRUE_WORD};

fn cf_string(s: &str) -> CFStringRef {
    unsafe { CFStringCreateWithBytes(kCFAllocatorDefault, s.as_ptr(), s.len() as isize, kCFStringEncodingUTF8, 0) }
}

fn copy_data(data: CFDataRef) -> Vec<u8> {
    if data.is_null() {
        return Vec::new();
    }
    unsafe {
        let len = CFDataGetLength(data).max(0) as usize;
        let ptr = CFDataGetBytePtr(data);
        if ptr.is_null() || len == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(ptr, len).to_vec()
        }
    }
}

/// The service's end of a port, which we send requests to.
struct RemotePort(CFMessagePortRef);

// SAFETY: a CFMessagePort may be used from any thread; the callers serialise
// access (a `Mutex` around the only shared instance).
unsafe impl Send for RemotePort {}
// SAFETY: Core Foundation message ports are thread-safe. The request port that
// is shared by a ServiceSession is additionally protected by a Mutex; the
// device-level port is only retained to keep the registration alive.
unsafe impl Sync for RemotePort {}

impl Drop for RemotePort {
    fn drop(&mut self) {
        unsafe {
            CFMessagePortInvalidate(self.0);
            CFRelease(self.0 as *const c_void);
        }
    }
}

impl RemotePort {
    fn open(name: &str, attempts: u32) -> Result<Self, String> {
        let cf_name = cf_string(name);
        let mut port = ptr::null_mut();
        for attempt in 0..attempts.max(1) {
            port = unsafe { CFMessagePortCreateRemote(kCFAllocatorDefault, cf_name) };
            if !port.is_null() {
                break;
            }
            if attempt + 1 < attempts {
                thread::sleep(Duration::from_millis(200));
            }
        }
        unsafe { CFRelease(cf_name as *const c_void) };
        if port.is_null() { Err(format!("could not open the NI service port '{name}'")) } else { Ok(Self(port)) }
    }

    /// Send one message and return a copy of the reply, which is empty when
    /// the service sends none. `Err(code)` is the `CFMessagePort` status.
    fn request(&self, message: &[u8], timeout: Duration) -> Result<Vec<u8>, i32> {
        unsafe {
            let data = CFDataCreate(kCFAllocatorDefault, message.as_ptr(), message.len() as isize);
            if data.is_null() {
                return Err(-4);
            }
            let mut reply: CFDataRef = ptr::null();
            let secs = timeout.as_secs_f64();
            let status = CFMessagePortSendRequest(self.0, 0, data, secs, secs, kCFRunLoopDefaultMode, &mut reply);
            CFRelease(data as *const c_void);
            let bytes = copy_data(reply);
            if !reply.is_null() {
                CFRelease(reply as *const c_void);
            }
            if status == kCFMessagePortSuccess { Ok(bytes) } else { Err(status) }
        }
    }

    /// Like [`request`], for the bootstrap and connect steps where a missing
    /// reply is an error.
    fn transact(&self, message: &[u8]) -> Result<Vec<u8>, String> {
        let reply = self
            .request(message, Duration::from_secs(2))
            .map_err(|code| format!("the NI service did not answer (status {code})"))?;
        if reply.is_empty() { Err("the NI service sent an empty reply".to_string()) } else { Ok(reply) }
    }
}

/// A notification port the service connects back to, served on its own
/// thread's run loop.
struct NotificationServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for NotificationServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Port callback. Copies the message and queues it; the empty reply is the
/// acknowledgement the service expects.
unsafe extern "C" fn on_notification(
    _local: CFMessagePortRef,
    _msgid: i32,
    data: CFDataRef,
    info: *mut c_void,
) -> CFDataRef {
    // SAFETY: `info` is the `Sender` the serving thread keeps alive for as long as the port exists.
    let events = unsafe { &*(info as *const Sender<Vec<u8>>) };
    let bytes = copy_data(data);
    tracing::trace!("NI service notification: {} bytes {:02x?}", bytes.len(), &bytes[..bytes.len().min(128)]);
    let _ = events.send(bytes);
    ptr::null()
}

fn serve_notifications(name: String, events: Sender<Vec<u8>>) -> Result<NotificationServer, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let (ready_tx, ready_rx) = channel::<Result<(), String>>();

    let thread = thread::Builder::new()
        .name("encdr-ni-notify".to_string())
        .spawn(move || {
            let events = Box::new(events);
            let info = &*events as *const Sender<Vec<u8>> as *mut c_void;
            let context =
                CFMessagePortContext { version: 0, info, retain: None, release: None, copyDescription: None };
            let cf_name = cf_string(&name);
            let mut should_free: Boolean = 0;
            let port = unsafe {
                CFMessagePortCreateLocal(kCFAllocatorDefault, cf_name, Some(on_notification), &context, &mut should_free)
            };
            unsafe { CFRelease(cf_name as *const c_void) };
            if port.is_null() {
                let _ = ready_tx.send(Err(format!("could not create the notification port '{name}'")));
                return;
            }
            let source = unsafe { CFMessagePortCreateRunLoopSource(kCFAllocatorDefault, port, 0) };
            let run_loop = unsafe { CFRunLoopGetCurrent() };
            if source.is_null() {
                unsafe {
                    CFMessagePortInvalidate(port);
                    CFRelease(port as *const c_void);
                }
                let _ = ready_tx.send(Err(format!("could not serve the notification port '{name}'")));
                return;
            }
            unsafe { CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode) };
            let _ = ready_tx.send(Ok(()));

            while !thread_stop.load(Ordering::Relaxed) {
                unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, 0) };
            }

            unsafe {
                CFRunLoopRemoveSource(run_loop, source, kCFRunLoopDefaultMode);
                CFMessagePortInvalidate(port);
                CFRelease(source as *const c_void);
                CFRelease(port as *const c_void);
            }
            drop(events);
        })
        .map_err(|e| e.to_string())?;

    let server = NotificationServer { stop, thread: Some(thread) };
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(server),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("the notification port did not start".to_string()),
    }
}

/// Register with the service for the bootstrap step shared by device and
/// instance connects, returning the request port and the notification server.
fn register(request: &[u8], events: &Sender<Vec<u8>>) -> Result<(RemotePort, NotificationServer), String> {
    let bootstrap = RemotePort::open(protocol::BOOTSTRAP_PIPE, 10)?;
    let reply = bootstrap.transact(request)?;
    drop(bootstrap);
    let ports = protocol::parse_connect_reply(&reply)?;
    let server = serve_notifications(ports.notification.clone(), events.clone())?;
    let request_port = RemotePort::open(&ports.request, 10)?;
    // The reply, if any, carries nothing we need.
    let _ = request_port.request(&protocol::ack_notification_port(&ports.notification), Duration::from_millis(500));
    Ok((request_port, server))
}

/// A registered client session for one physical device.
pub struct ServiceSession {
    /// Held open so the service keeps the device-level registration.
    _device: (RemotePort, NotificationServer),
    instance: Mutex<RemotePort>,
    _instance_notifications: NotificationServer,
    control: async_channel::Receiver<bool>,
}

impl ServiceSession {
    /// Connect to the service and register for the unit with the given
    /// serial number (or the first one it reports when `serial` is `None` or
    /// not found).
    pub fn connect(product_id: u16, serial: Option<&str>, timeout: Duration) -> Result<Self, String> {
        if RemotePort::open(protocol::BOOTSTRAP_PIPE, 1).is_err() {
            return Err("the NI hardware service is not running".to_string());
        }

        let (tx, rx) = channel::<Vec<u8>>();
        let device = register(&protocol::device_connect(product_id), &tx)?;
        let _ = device.0.request(&protocol::device_state_request(), Duration::from_millis(500));

        let serial = super::pick_serial(&rx, serial, timeout)?;
        tracing::info!("NI service reports device {serial}; registering for its screens");
        let control = super::watch_control(rx);

        let (instance, notifications) = register(&protocol::instance_connect(product_id, &serial), &tx)?;
        for message in protocol::instance_setup(product_id) {
            // Some setup messages are answered and some are not; none is needed.
            let _ = instance.request(&message, Duration::from_millis(40));
        }

        Ok(Self { _device: device, instance: Mutex::new(instance), _instance_notifications: notifications, control })
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
        let port = self.instance.lock().map_err(|_| "NI service session poisoned".to_string())?;
        match port.request(&message, protocol::DISPLAY_REPLY_TIMEOUT) {
            Ok(reply) if reply.get(..4).is_some_and(|w| w == TRUE_WORD.to_le_bytes()) => Ok(()),
            Ok(reply) => Err(format!("{} (reply {:02x?})", super::REJECTED_FRAME,&reply[..reply.len().min(8)])),
            Err(code) => Err(format!("the NI service did not accept a frame (status {code})")),
        }
    }
}

//! Messages exchanged with the NI hardware service over named pipes.
//!
//! Every message is a sequence of little-endian 32-bit words, optionally
//! followed by raw bytes. This module only builds and parses byte buffers, so
//! it compiles and is tested on every platform; the pipe I/O lives in
//! `windows.rs`.
//!
//! Status: the connection sequence and the screen message are verified on
//! hardware (Windows, Maschine Mk3). Nothing else is used.

use std::time::Duration;

/// Primary bootstrap pipe/port (`NIHostIntegrationAgent`).
pub const BOOTSTRAP_PIPE: &str = "com.native-instruments.NIHostIntegrationAgent";

/// Bootstrap pipes/ports exposed by NI's background services, tried in order:
/// `NIHostIntegrationAgent` first (Maschine Mk3, Maschine Plus, etc.), then
/// `NIHardwareService` / `NIHardwareAgent` (`NIHWMainHandler`, used by
/// Maschine Studio and standalone NI hardware service installations).
pub const BOOTSTRAP_PIPES: &[&str] = &[
    BOOTSTRAP_PIPE,
    "NIHWMainHandler",
];

/// The service's "yes" word, which also opens every successful reply.
pub const TRUE_WORD: u32 = 0x7472_7565;

const CLIENT_KIND: u32 = 0x4e69_4d32;
const CLIENT_ROLE: u32 = 0x7072_6d79;

const MSG_VERSION: u32 = 0x0353_6756;
const MSG_DEVICE_CONNECT: u32 = 0x0344_7500;
const MSG_INSTANCE_CONNECT: u32 = 0x0344_4900;
const MSG_ACK_NOTIFICATION_PORT: u32 = 0x0340_4300;
const MSG_DEVICE_STATE_REQUEST: u32 = 0x0344_7143;
const MSG_DISPLAY: u32 = 0x0364_7344;

const NOTE_DEVICE_ON: u32 = 0x0344_4e2b;
const NOTE_DEVICE_OFF: u32 = 0x0344_4e2d;

/// Sent on an instance's notification port whenever it gains or loses the
/// hardware (the controller's instance button): `TRUE` or `0` follows.
const NOTE_CONTROL: u32 = 0x0343_4e00;

/// How long to wait for the service to answer a screen frame.
pub const DISPLAY_REPLY_TIMEOUT: Duration = Duration::from_millis(1500);

fn words(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// Service reachability probe, sent on the bootstrap pipe.
pub fn version_probe() -> Vec<u8> {
    words(&[MSG_VERSION])
}

/// Register as a client of every device with this product id.
pub fn device_connect(product_id: u16) -> Vec<u8> {
    words(&[MSG_DEVICE_CONNECT, product_id as u32, CLIENT_KIND, CLIENT_ROLE, 0])
}

/// Register as a client of one physical unit. The service counts the serial's
/// trailing NUL in the length, and refuses the request without it.
pub fn instance_connect(product_id: u16, serial: &str) -> Vec<u8> {
    let mut v = words(&[
        MSG_INSTANCE_CONNECT,
        product_id as u32,
        CLIENT_KIND,
        CLIENT_ROLE,
        serial.len() as u32 + 1,
    ]);
    v.extend_from_slice(serial.as_bytes());
    v.push(0);
    v
}

/// Tell the service the name of the pipe we serve notifications on.
pub fn ack_notification_port(name: &str) -> Vec<u8> {
    let mut v = words(&[MSG_ACK_NOTIFICATION_PORT, TRUE_WORD, 0, name.len() as u32]);
    v.extend_from_slice(name.as_bytes());
    v.push(0);
    v
}

pub fn device_state_request() -> Vec<u8> {
    words(&[MSG_DEVICE_STATE_REQUEST])
}

/// Messages an instance sends on its request port before it draws. The
/// service answers some of them; none is needed to be read.
pub fn instance_setup(product_id: u16) -> Vec<Vec<u8>> {
    let project = b"Encdr\0";
    let mut project_msg = words(&[0x0349_734e, 0x7000_1006, 0xf6b2_4000, project.len() as u32]);
    project_msg.extend_from_slice(project);
    vec![
        words(&[0x0356_6775, 0x4b65_7973]),
        words(&[0x0356_6775, 0x524b_6579]),
        words(&[0x0343_6753]),
        project_msg,
        words(&[0x0344_674d, product_id as u32]),
        words(&[0x0343_4300, 0x7374_7274]),
        words(&[0x0349_7354, 0]),
        words(&[0x0343_6746]),
        words(&[0x0349_7374, TRUE_WORD]),
        words(&[0x0344_5200, 2]),
        words(&[0x0344_4100, 3, 0, 0x04e2_97b0]),
    ]
}

/// Wrap a screen command stream (the same bytes a USB bulk transfer would
/// carry) for display `display`.
pub fn display_message(display: u8, width: u16, height: u16, commands: &[u8]) -> Vec<u8> {
    let size = ((width as u32) << 16) | height as u32;
    let mut v = words(&[MSG_DISPLAY, display as u32, 0, size, commands.len() as u32]);
    v.extend_from_slice(commands);
    v
}

fn le32(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn text(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).trim_end_matches('\0').to_string()
}

/// Port names returned when connecting as a client.
#[derive(Debug, PartialEq, Eq)]
pub struct Ports {
    pub request: String,
    pub notification: String,
}

/// Reply layout: `TRUE, len, name\0, len, name\0`.
pub fn parse_connect_reply(d: &[u8]) -> Result<Ports, String> {
    if le32(d, 0) != Some(TRUE_WORD) {
        return Err(format!(
            "the NI service refused the connection (reply {:02x?})",
            &d[..d.len().min(16)]
        ));
    }
    let req_len = le32(d, 4).ok_or("short reply")? as usize;
    let request = d.get(8..8 + req_len).ok_or("short reply (request port)")?;
    let note_len = le32(d, 8 + req_len).ok_or("short reply (notification length)")? as usize;
    let notification = d.get(12 + req_len..12 + req_len + note_len).ok_or("short reply (notification port)")?;
    Ok(Ports { request: text(request), notification: text(notification) })
}

/// Device present (`true`) or gone, with the unit's serial number.
#[derive(Debug, PartialEq, Eq)]
pub struct DeviceState {
    pub present: bool,
    pub serial: String,
}

pub fn parse_device_state(d: &[u8]) -> Option<DeviceState> {
    let present = match le32(d, 0)? {
        NOTE_DEVICE_ON => true,
        NOTE_DEVICE_OFF => false,
        _ => return None,
    };
    let len = le32(d, 12)? as usize;
    let raw = d.get(16..16usize.checked_add(len)?)?;
    Some(DeviceState { present, serial: text(raw) })
}

/// Whether this client has the hardware, from a control notification:
/// `Some(true)` when it became the active instance, `Some(false)` when another
/// client (NI's software) took over, `None` for any other message.
pub fn parse_control_state(d: &[u8]) -> Option<bool> {
    if le32(d, 0)? != NOTE_CONTROL {
        return None;
    }
    Some(le32(d, 4)? == TRUE_WORD)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Byte sequences below are from a real session on a Maschine Mk3.

    #[test]
    fn parses_control_notifications() {
        // Captured while switching between Maschine and Encdr with the instance button.
        assert_eq!(parse_control_state(&[0x00, 0x4e, 0x43, 0x03, 0x65, 0x75, 0x72, 0x74]), Some(true));
        assert_eq!(parse_control_state(&[0x00, 0x4e, 0x43, 0x03, 0, 0, 0, 0]), Some(false));
        // Other instance notifications are not control changes.
        assert_eq!(parse_control_state(&[0x00, 0x4e, 0x44, 0x03, 0x65, 0x75, 0x72, 0x74]), None);
        let button = [
            0x00, 0x4e, 0x73, 0x03, 0xa4, 0x07, 0, 0, 0x08, 0x7b, 0x3d, 0x66, 1, 0, 0, 0, 6, 0, 0, 0, 1, 0xd5, 0, 0,
        ];
        assert_eq!(parse_control_state(&button), None);
        assert_eq!(parse_control_state(&[0x00, 0x4e, 0x43, 0x03]), None, "truncated");
    }

    #[test]
    fn device_connect_matches_the_wire_format() {
        assert_eq!(
            device_connect(0x1600),
            [0x00, 0x75, 0x44, 0x03, 0x00, 0x16, 0x00, 0x00, 0x32, 0x4d, 0x69, 0x4e, 0x79, 0x6d, 0x72, 0x70, 0, 0, 0, 0]
        );
    }

    #[test]
    fn instance_connect_sends_the_serial_with_its_nul() {
        let m = instance_connect(0x1600, "F8D8CDD9");
        assert_eq!(&m[..4], [0x00, 0x49, 0x44, 0x03]);
        assert_eq!(&m[16..20], [9, 0, 0, 0], "length counts the NUL");
        assert_eq!(&m[20..], b"F8D8CDD9\0");
    }

    #[test]
    fn ack_names_the_notification_pipe() {
        let m = ack_notification_port("NIHWS16000019Notification");
        assert_eq!(&m[..8], [0x00, 0x43, 0x40, 0x03, 0x65, 0x75, 0x72, 0x74]);
        assert_eq!(le32(&m, 12), Some(25));
        assert_eq!(m.len(), 16 + 25 + 1);
        assert_eq!(*m.last().unwrap(), 0);
    }

    #[test]
    fn display_message_has_the_screen_header() {
        let m = display_message(0, 480, 272, &[0x84, 0x00, 0x00, 0x60]);
        assert_eq!(
            &m[..20],
            [0x44, 0x73, 0x64, 0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x01, 0xe0, 0x01, 4, 0, 0, 0]
        );
        assert_eq!(&m[20..], [0x84, 0x00, 0x00, 0x60]);
        assert_eq!(display_message(1, 480, 272, &[])[4], 1, "second screen");
    }

    #[test]
    fn display_message_handles_traktor_s4_mk3_dimensions() {
        let m = display_message(0, 320, 240, &[0x84, 0x00, 0x00, 0x60]);
        assert_eq!(
            &m[..20],
            [0x44, 0x73, 0x64, 0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0xf0, 0x00, 0x40, 0x01, 4, 0, 0, 0]
        );
        assert_eq!(&m[20..], [0x84, 0x00, 0x00, 0x60]);
        assert_eq!(display_message(1, 320, 240, &[])[4], 1, "second screen");
    }

    #[test]
    fn parses_a_connect_reply() {
        let mut reply = words(&[TRUE_WORD, 21]);
        reply.extend_from_slice(b"NIHWS16000019Request\0");
        reply.extend_from_slice(&words(&[26]));
        reply.extend_from_slice(b"NIHWS16000019Notification\0");
        assert_eq!(
            parse_connect_reply(&reply),
            Ok(Ports { request: "NIHWS16000019Request".into(), notification: "NIHWS16000019Notification".into() })
        );
    }

    #[test]
    fn a_refusal_is_an_error() {
        assert!(parse_connect_reply(&[0x64, 0x69, 0x63, 0x65]).is_err());
        assert!(parse_connect_reply(&[]).is_err());
    }

    #[test]
    fn parses_device_state_notifications() {
        let mut on = words(&[NOTE_DEVICE_ON, 0, 0x1600, 9]);
        on.extend_from_slice(b"F8D8CDD9\0");
        assert_eq!(parse_device_state(&on), Some(DeviceState { present: true, serial: "F8D8CDD9".into() }));

        let mut off = words(&[NOTE_DEVICE_OFF, 0x5f, 0x1600, 9]);
        off.extend_from_slice(b"F8D8CDD9\0");
        assert_eq!(parse_device_state(&off).map(|s| s.present), Some(false));

        assert_eq!(parse_device_state(&words(&[0x0344_4e00, 1])), None);
        assert_eq!(parse_device_state(&words(&[NOTE_DEVICE_ON, 0, 0, 9])), None, "truncated serial");
    }
}

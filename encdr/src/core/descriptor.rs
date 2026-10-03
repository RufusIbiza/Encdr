use serde::Deserialize;
use serde::Deserializer;
use std::collections::HashMap;

/// Top-level device descriptor, parsed from JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceDescriptor {
    pub name: String,
    pub manufacturer: String,
    pub vendor_id: HexU16,
    pub product_id: HexU16,
    pub interfaces: Vec<InterfaceDesc>,
    pub input_packets: Vec<InputPacketDesc>,
    #[serde(default, deserialize_with = "deserialize_leds")]
    pub leds: Vec<LedLayoutDesc>,
    #[serde(default)]
    pub screens: Vec<ScreenDesc>,
    #[serde(default)]
    pub quirks: QuirksDesc,
}

impl DeviceDescriptor {
    /// Find an interface descriptor by its string id.
    pub fn interface_by_id(&self, id: &str) -> Option<&InterfaceDesc> {
        self.interfaces.iter().find(|i| i.id == id)
    }

    /// Count total number of named controls (buttons + sliders + encoders).
    pub fn control_count(&self) -> usize {
        self.input_packets
            .iter()
            .flat_map(|p| &p.items)
            .count()
    }

    /// Iterate all input item descriptors across all packets.
    pub fn all_inputs(&self) -> impl Iterator<Item = &InputItemDesc> {
        self.input_packets.iter().flat_map(|p| &p.items)
    }
}

/// Deserialize `leds` as either a single object or an array of objects.
fn deserialize_leds<'de, D>(deserializer: D) -> Result<Vec<LedLayoutDesc>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(LedLayoutDesc),
        Many(Vec<LedLayoutDesc>),
    }
    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(single) => Ok(vec![single]),
        OneOrMany::Many(vec) => Ok(vec),
    }
}

// ── Hex u16 helper for JSON "0x17cc" style values ──────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HexU16(pub u16);

impl<'de> Deserialize<'de> for HexU16 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let val = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u16::from_str_radix(hex, 16).map_err(serde::de::Error::custom)?
        } else {
            s.parse::<u16>().map_err(serde::de::Error::custom)?
        };
        Ok(HexU16(val))
    }
}

// ── USB interface ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct InterfaceDesc {
    pub id: String,
    pub number: u8,
    /// Alternate setting to select after claiming the interface (e.g. the
    /// Maschine Mk1 only exposes its pad and display endpoints in alt 1).
    #[serde(default)]
    pub alt_setting: Option<u8>,
    pub endpoints: EndpointMap,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndpointMap {
    #[serde(rename = "in")]
    pub ep_in: Option<EndpointDesc>,
    pub out: Option<EndpointDesc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndpointDesc {
    pub address: HexU16,
    #[serde(rename = "type")]
    pub transfer_type: TransferType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferType {
    Interrupt,
    Bulk,
    Control,
    Isochronous,
}

// ── Input packets ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct InputPacketDesc {
    pub id: String,
    pub interface: String,
    pub size: usize,
    /// Leading byte that identifies this packet. When any packet on an
    /// interface declares a `report_id` or `pad_format`, packets on that
    /// interface are routed by these fields instead of by size.
    #[serde(default)]
    pub report_id: Option<HexU16>,
    /// Pad stream encoding for packets that carry pad pressure data.
    #[serde(default)]
    pub pad_format: Option<PadFormat>,
    pub items: Vec<InputItemDesc>,
}

impl InputPacketDesc {
    /// Whether this packet is routed by report id / pad format rather than size.
    pub fn is_routed(&self) -> bool {
        self.report_id.is_some() || self.pad_format.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadFormat {
    /// Stream of little-endian u16 words: bits 15..12 are the hardware pad
    /// index, bits 11..0 the pressure (0..4095). Words are self-identifying,
    /// so frames need not align with USB packet boundaries (Maschine Mk1).
    IdPressureWords,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItemDesc {
    Button(ButtonItemDesc),
    Slider(SliderItemDesc),
    Encoder(EncoderItemDesc),
    EncoderFine(EncoderFineItemDesc),
    Touch(TouchItemDesc),
}

impl InputItemDesc {
    pub fn name(&self) -> &str {
        match self {
            InputItemDesc::Button(b) => &b.name,
            InputItemDesc::Slider(s) => &s.name,
            InputItemDesc::Encoder(e) => &e.name,
            InputItemDesc::EncoderFine(e) => &e.name,
            InputItemDesc::Touch(t) => &t.name,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ButtonItemDesc {
    pub name: String,
    pub byte: usize,
    pub mask: HexU16,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SliderItemDesc {
    pub name: String,
    /// Single byte index or multi-byte array
    #[serde(default)]
    pub byte: Option<usize>,
    #[serde(default)]
    pub bytes: Option<Vec<usize>>,
    #[serde(default = "default_bits")]
    pub bits: u8,
    #[serde(default)]
    pub normalize: bool,
    #[serde(default)]
    pub max_value: Option<u32>,
}

fn default_bits() -> u8 {
    16
}

#[derive(Debug, Clone, Deserialize)]
pub struct EncoderItemDesc {
    pub name: String,
    pub byte: usize,
    #[serde(default = "default_encoder_bits")]
    pub bits: u8,
    #[serde(default)]
    pub bit_offset: u8,
    pub encoding: EncoderEncoding,
}

fn default_encoder_bits() -> u8 {
    4
}

#[derive(Debug, Clone, Deserialize)]
pub struct EncoderFineItemDesc {
    pub name: String,
    pub bytes: Vec<usize>,
    pub encoding: EncoderEncoding,
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Minimum raw movement before a delta is emitted (`erp` encoding only).
    #[serde(default)]
    pub deadband: u16,
}

fn default_scale() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct TouchItemDesc {
    pub name: String,
    /// Single byte index (for 1-byte bitmask touch detection)
    #[serde(default)]
    pub byte: Option<usize>,
    /// Multi-byte indices (for wide touch detection, e.g. 16-bit value > 0)
    #[serde(default)]
    pub bytes: Option<Vec<usize>>,
    /// Bitmask — for single-byte touch, applied as `buf[byte] & mask != 0`.
    /// For multi-byte touch, if omitted the value is simply checked for `> 0`.
    #[serde(default)]
    pub mask: Option<HexU16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncoderEncoding {
    /// 4-bit counter with wraparound (notched encoders like D2 browse/loop)
    Wrap16,
    /// 16-bit signed delta (high-res encoders like D2 screen encoders)
    Signed16,
    /// 16-bit unsigned absolute position
    Unsigned16,
    /// 16-bit counter with full wraparound (jogwheels/jogdials)
    /// Reports delta via shortest-path around the 65536-step ring, scaled by the
    /// encoder's `scale` factor.
    Wrap16Wide,
    /// Endless rotary potentiometer: two 8-bit analog taps 90° apart
    /// (`bytes: [b, a]`), decoded to an absolute 0..999 position per turn.
    /// Reports delta via shortest path around the 1000-step ring, scaled by
    /// the encoder's `scale` factor (NI Maschine Mk1, Kore).
    Erp,
}

// ── LED layout ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct LedLayoutDesc {
    #[serde(default)]
    pub id: String,
    pub interface: String,
    pub buffer_size: usize,
    /// Single byte written before the LED data. Ignored when `prefix` is set.
    #[serde(default)]
    pub prefix_byte: HexU16,
    /// Multi-byte header written before the LED data (e.g. `["0x0c", "0x1e"]`
    /// for a command byte plus bank offset).
    #[serde(default)]
    pub prefix: Vec<HexU16>,
    pub items: Vec<LedItemDesc>,
}

impl LedLayoutDesc {
    /// Bytes written before the LED data on every flush.
    pub fn prefix_bytes(&self) -> Vec<u8> {
        if self.prefix.is_empty() {
            vec![self.prefix_byte.0 as u8]
        } else {
            self.prefix.iter().map(|b| b.0 as u8).collect()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LedItemDesc {
    Rgb(RgbLedDesc),
    Single(SingleLedDesc),
    Strip(StripLedDesc),
    Indexed(SingleLedDesc),
}

impl LedItemDesc {
    pub fn name(&self) -> &str {
        match self {
            LedItemDesc::Rgb(r) => &r.name,
            LedItemDesc::Single(s) => &s.name,
            LedItemDesc::Strip(s) => &s.name,
            LedItemDesc::Indexed(i) => &i.name,
        }
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct RgbLedDesc {
    pub name: String,
    pub offsets: RgbOffsets,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RgbOffsets {
    pub r: usize,
    pub g: usize,
    pub b: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SingleLedDesc {
    pub name: String,
    pub offset: usize,
    /// Value written on connect, before any application LED updates
    /// (e.g. a display backlight that should start on).
    #[serde(default)]
    pub default: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StripLedDesc {
    pub name: String,
    pub offset: usize,
    pub count: usize,
}

// ── Screen ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ScreenDesc {
    pub name: String,
    pub interface: String,
    pub width: u16,
    pub height: u16,
    pub pixel_format: PixelFormat,
    #[serde(default)]
    pub full_blit: ScreenBlitDesc,
    #[serde(default)]
    pub partial_blit: Option<PartialBlitDesc>,
    /// Controller-specific framing and init. When absent, a frame is sent as
    /// a single `full_blit` transfer.
    #[serde(default)]
    pub protocol: Option<ScreenProtocol>,
}

impl ScreenDesc {
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn byte_size(&self) -> usize {
        match self.pixel_format {
            PixelFormat::Mono => (self.pixel_count() + 7) / 8,
            PixelFormat::St7529Gray5 => {
                self.width.div_ceil(3) as usize * 2 * self.height as usize
            }
            _ => self.pixel_count() * self.pixel_format.bytes_per_pixel(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScreenProtocol {
    /// Sitronix ST7529 behind NI's EP8 display bridge (Maschine Mk1). Each
    /// transfer is `[display << 1 | data_flag, len_hi, len_lo, payload…]`.
    /// The controller is initialised on connect and frames are sent as a
    /// RAMWR command followed by ≤502-byte data chunks.
    NiSt7529 { display: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    Bgr565Be,
    Rgb565Le,
    Rgb888,
    Rgba8888,
    Mono,
    /// ST7529 32-level grayscale, 3 pixels packed into 2 bytes
    /// (`[p0:5 p1_hi:3] [p1_lo:2 _:1 p2:5]`), stored inverted (0 = lit).
    St7529Gray5,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Bgr565Be | PixelFormat::Rgb565Le => 2,
            PixelFormat::Rgb888 => 3,
            PixelFormat::Rgba8888 => 4,
            PixelFormat::Mono => 1, // 1 byte per 8 pixels, but we treat per-pixel
            PixelFormat::St7529Gray5 => 1, // 2 bytes per 3 pixels, but we treat per-pixel
        }
    }

    /// Native byte value that fills a frame with black.
    pub fn black_fill(&self) -> u8 {
        match self {
            PixelFormat::St7529Gray5 => 0xFF,
            _ => 0x00,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenBlitDesc {
    pub header: String,
    pub footer: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartialBlitDesc {
    pub supported: bool,
    pub x_align: u16,
    pub y_align: u16,
    pub header_template: String,
    pub footer: String,
}

// ── Quirks ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QuirksDesc {
    #[serde(default)]
    pub dual_handle: bool,
    #[serde(default)]
    pub detach_kernel_driver: bool,
    #[serde(default)]
    pub touchstrip: Option<TouchstripQuirksDesc>,
    #[serde(default)]
    pub feature_report_leds: Option<FeatureReportLedsQuirkDesc>,
    /// Raw writes sent once on connect, after input reads are queued (so a
    /// device that stalls commands until replies are read cannot deadlock).
    #[serde(default)]
    pub init_writes: Vec<InitWriteDesc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitWriteDesc {
    /// Interface id whose OUT endpoint receives the write.
    pub interface: String,
    /// Comma-separated byte list, e.g. `"0x0b,0x01,0x0a,0x05"`.
    pub data: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TouchstripQuirksDesc {
    #[serde(default)]
    pub heartbeat_bytes: HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureReportLedsQuirkDesc {
    pub report_id: HexU16,
    pub interface: usize,
    #[serde(default = "default_feature_report_len")]
    pub payload_length: usize,
    pub items: HashMap<String, FeatureReportLedItemDesc>,
}

fn default_feature_report_len() -> usize {
    33
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureReportLedItemDesc {
    pub command: HexU16,
    pub mask: HexU16,
}

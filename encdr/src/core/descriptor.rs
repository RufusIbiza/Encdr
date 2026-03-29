use serde::Deserialize;
use serde::Deserializer;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub items: Vec<InputItemDesc>,
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
}

// ── LED layout ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct LedLayoutDesc {
    #[serde(default)]
    pub id: String,
    pub interface: String,
    pub buffer_size: usize,
    pub prefix_byte: HexU16,
    pub items: Vec<LedItemDesc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LedItemDesc {
    Rgb(RgbLedDesc),
    Single(SingleLedDesc),
    Strip(StripLedDesc),
}

impl LedItemDesc {
    pub fn name(&self) -> &str {
        match self {
            LedItemDesc::Rgb(r) => &r.name,
            LedItemDesc::Single(s) => &s.name,
            LedItemDesc::Strip(s) => &s.name,
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
    pub full_blit: ScreenBlitDesc,
    #[serde(default)]
    pub partial_blit: Option<PartialBlitDesc>,
}

impl ScreenDesc {
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn byte_size(&self) -> usize {
        self.pixel_count() * self.pixel_format.bytes_per_pixel()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    Bgr565Be,
    Rgb565Le,
    Rgb888,
    Rgba8888,
    Mono,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Bgr565Be | PixelFormat::Rgb565Le => 2,
            PixelFormat::Rgb888 => 3,
            PixelFormat::Rgba8888 => 4,
            PixelFormat::Mono => 1, // 1 byte per 8 pixels, but we treat per-pixel
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
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
}

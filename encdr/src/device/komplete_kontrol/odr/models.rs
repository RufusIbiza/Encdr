//! Strongly typed data models for Komplete Kontrol Mk3 On-Device Rendering (ODR).

use serde::{Deserialize, Serialize};

/// RGB color struct serialized for ODR layouts and themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default)]
    pub is_rgb565: bool,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            is_rgb565: false,
        }
    }

    pub const RED: Self = Self::new(255, 30, 30);
    pub const ORANGE: Self = Self::new(255, 128, 0);
    pub const YELLOW: Self = Self::new(255, 220, 0);
    pub const GREEN: Self = Self::new(40, 220, 80);
    pub const CYAN: Self = Self::new(0, 210, 255);
    pub const BLUE: Self = Self::new(30, 100, 255);
    pub const PURPLE: Self = Self::new(160, 40, 255);
    pub const MAGENTA: Self = Self::new(255, 40, 180);
    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const OFF: Self = Self::new(0, 0, 0);
}

/// Primary screen view templates on the Komplete Kontrol Mk3 hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KkMk3Page {
    /// Active instrument/effect parameter controls.
    Parameters,
    /// Sound & preset browser with category filters.
    Browser,
    /// Multi-track mixer view with VU meters and channel strips.
    Mixer,
    /// Scales, chords, and arpeggiator engine view.
    SmartPlay,
    /// Multi-device insert chain view.
    PluginChain,
    /// Hardware settings and preferences.
    Settings,
}

/// Hardware foot pedal jack connection identifier on Komplete Kontrol Mk3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PedalType {
    /// Sustain pedal input (Jack 1).
    Sustain,
    /// Expression / continuous pedal input (Jack 2).
    Expression,
    /// Assignable pedal input 1 (Jack 3).
    Assign1,
    /// Assignable pedal input 2 (Jack 4).
    Assign2,
}

impl PedalType {
    pub fn as_sym(&self) -> &'static str {
        match self {
            Self::Sustain => "pedal_sustain_connected",
            Self::Expression => "pedal_expression_connected",
            Self::Assign1 => "pedal_assign1_connected",
            Self::Assign2 => "pedal_assign2_connected",
        }
    }

    pub fn from_sym(name: &str) -> Option<Self> {
        match name {
            "pedal_sustain_connected" => Some(Self::Sustain),
            "pedal_expression_connected" => Some(Self::Expression),
            "pedal_assign1_connected" => Some(Self::Assign1),
            "pedal_assign2_connected" => Some(Self::Assign2),
            _ => None,
        }
    }
}

/// Native oscillator waveform glyph identifier supported by the Komplete Kontrol Mk3 display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaveformType {
    #[default]
    Default,
    Sine,
    Square,
    Square2,
    Square3,
    Square4,
    Triangle,
    Triangle2,
    Triangle3,
    Triangle4,
    Saw,
    Saw2,
    Saw3,
    Saw4,
    Noise,
    Pwm,
    Wavetable,
    Wavetable2,
    Wavetable3,
    Wavetable4,
    Complex,
}

impl WaveformType {
    pub fn as_sym(&self) -> &'static str {
        match self {
            Self::Default => "default_waveform",
            Self::Sine => "sine",
            Self::Square => "square",
            Self::Square2 => "square_2",
            Self::Square3 => "square_3",
            Self::Square4 => "square_4",
            Self::Triangle => "triangle",
            Self::Triangle2 => "triangle_2",
            Self::Triangle3 => "triangle_3",
            Self::Triangle4 => "triangle_4",
            Self::Saw => "saw",
            Self::Saw2 => "saw_2",
            Self::Saw3 => "saw_3",
            Self::Saw4 => "saw_4",
            Self::Noise => "noise",
            Self::Pwm => "pwm",
            Self::Wavetable => "wavetable",
            Self::Wavetable2 => "wavetable_2",
            Self::Wavetable3 => "wavetable_3",
            Self::Wavetable4 => "wavetable_4",
            Self::Complex => "complex",
        }
    }
}

/// Native filter frequency response curve identifier supported by the Komplete Kontrol Mk3 display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    #[default]
    Default,
    HiPass,
    ResonantHiPass,
    LoPass,
    ResonantLoPass,
    BandPass,
    AllPass,
    Bell,
    PropQ,
    BandShelf,
    AnalogLoShelf,
    AnalogHiShelf,
    BaxandallLoShelf,
    BaxandallHiShelf,
    Comb,
    BandReject,
}

impl FilterType {
    pub fn as_sym(&self) -> &'static str {
        match self {
            Self::Default => "default_filter_type",
            Self::HiPass => "hi_pass",
            Self::ResonantHiPass => "resonant_hi_pass",
            Self::LoPass => "lo_pass",
            Self::ResonantLoPass => "resonant_lo_pass",
            Self::BandPass => "band_pass",
            Self::AllPass => "all_pass",
            Self::Bell => "bell",
            Self::PropQ => "prop_q",
            Self::BandShelf => "band_shelf",
            Self::AnalogLoShelf => "analog_lo_shelf",
            Self::AnalogHiShelf => "analog_hi_shelf",
            Self::BaxandallLoShelf => "baxandall_lo_shelf",
            Self::BaxandallHiShelf => "baxandall_hi_shelf",
            Self::Comb => "comb",
            Self::BandReject => "band_reject",
        }
    }
}

/// Visual parameter presentation style rendered above and below rotary encoders on the Mk3 display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParameterStyle {
    /// Continuous rotary arc gauge.
    #[default]
    Knob,
    /// Numeric or text readout without an arc.
    Value,
    /// On/off power push-button indicator.
    Power,
    /// Musical beat tempo-synced value (e.g. 1/4, 1/8D).
    TempoSync,
    /// Stepped menu or dropdown selection item.
    Menu,
    /// Native oscillator waveform shape vector (sine, saw, square, etc.).
    Waveform(WaveformType),
    /// Native filter frequency response curve (lowpass, highpass, resonant, etc.).
    Filter(FilterType),
}

impl ParameterStyle {
    pub fn as_sym(&self) -> &'static str {
        match self {
            Self::Knob => "knob_parameter_style",
            Self::Value => "value_parameter_style",
            Self::Power => "power_parameter_style",
            Self::TempoSync => "tempo_sync_parameter_style",
            Self::Menu => "menu_parameter_style",
            Self::Waveform(_) => "waveform_parameter_style",
            Self::Filter(_) => "filtertype_parameter_style",
        }
    }
}

/// Visual control widget representation rendered above and below the physical rotary encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetDisplayType {
    /// Radial rotary arc gauge.
    Knob,
    /// Linear / continuous range slider.
    Range,
    /// 2-state on/off toggle button.
    Toggle,
    /// Momentary trigger / hit action.
    Trigger,
    /// Discrete stepped list selector (e.g. wave shapes, octaves).
    Increment,
    /// Bipolar center-detented indicator (-1.0 .. +1.0).
    Relative,
    /// Static text label.
    Text,
    /// Inactive / unassigned knob slot.
    Disabled,
}

/// Single parameter definition within a parameter page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterItem {
    /// Parameter title (e.g. `"Cutoff"`, `"Resonance"`).
    pub name: String,
    /// Normalized value in `0.0..=1.0` (or `-1.0..=1.0` for bipolar).
    pub value: f32,
    /// Formatted display readout string (e.g. `"1.2 kHz"`, `"-6.0 dB"`, `"45%"`).
    pub display_value: String,
    /// Visual widget template style.
    pub display_type: WidgetDisplayType,
    /// Group / section heading label (e.g. `"Filter"`, `"Amp Env"`).
    #[serde(default)]
    pub section_name: String,
    /// Native parameter rendering style on the Mk3 display.
    #[serde(default)]
    pub style: ParameterStyle,
}

impl ParameterItem {
    pub fn new(
        name: impl Into<String>,
        value: f32,
        display_value: impl Into<String>,
        display_type: WidgetDisplayType,
        section_name: impl Into<String>,
    ) -> Self {
        let style = match display_type {
            WidgetDisplayType::Toggle => ParameterStyle::Power,
            _ => ParameterStyle::Knob,
        };
        Self {
            name: name.into(),
            value,
            display_value: display_value.into(),
            display_type,
            section_name: section_name.into(),
            style,
        }
    }

    /// Set an explicit visual parameter style (e.g. native waveform or filter graphic).
    pub fn with_style(mut self, style: ParameterStyle) -> Self {
        self.style = style;
        self
    }

    /// Convenience builder for a standard rotary knob.
    pub fn knob(name: impl Into<String>, value: f32, display_value: impl Into<String>, section: impl Into<String>) -> Self {
        Self::new(name, value, display_value, WidgetDisplayType::Knob, section)
    }

    /// Convenience builder for a 2-state toggle switch.
    pub fn toggle(name: impl Into<String>, on: bool, section: impl Into<String>) -> Self {
        Self::new(
            name,
            if on { 1.0 } else { 0.0 },
            if on { "On" } else { "Off" },
            WidgetDisplayType::Toggle,
            section,
        )
    }

    /// Convenience builder for a parameter rendering a native oscillator waveform shape.
    pub fn waveform(name: impl Into<String>, waveform: WaveformType, section: impl Into<String>) -> Self {
        Self::new(name, 0.0, waveform.as_sym(), WidgetDisplayType::Increment, section)
            .with_style(ParameterStyle::Waveform(waveform))
    }

    /// Convenience builder for a parameter rendering a native filter response curve.
    pub fn filter(
        name: impl Into<String>,
        cutoff: f32,
        display_value: impl Into<String>,
        filter: FilterType,
        section: impl Into<String>,
    ) -> Self {
        Self::new(name, cutoff, display_value, WidgetDisplayType::Knob, section)
            .with_style(ParameterStyle::Filter(filter))
    }
}

/// Top-level model for an active plugin / parameter page.
///
/// Drives the header image banner, color theme, and parameter controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginData {
    /// Plugin / instrument title displayed in the header.
    pub name: String,
    /// Identifier of the image asset displayed across the top header banner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Primary RGB color theme for gauges, buttons, and accents.
    pub plugin_color: RgbColor,
    /// Secondary accent color code.
    #[serde(default)]
    pub nks_control_color: u8,
    /// Up to 8 parameter definitions mapped to the physical encoders.
    pub parameters: Vec<ParameterItem>,
    /// Color encoding flag.
    #[serde(default)]
    pub is_rgb565: bool,
}

impl PluginData {
    pub fn new(name: impl Into<String>, color: RgbColor) -> Self {
        Self {
            name: name.into(),
            background: None,
            plugin_color: color,
            nks_control_color: 1,
            parameters: Vec::new(),
            is_rgb565: false,
        }
    }

    /// Set the top header banner image asset identifier.
    pub fn with_background(mut self, asset_id: impl Into<String>) -> Self {
        self.background = Some(asset_id.into());
        self
    }

    /// Add a parameter control to this page.
    pub fn add_parameter(&mut self, param: ParameterItem) -> &mut Self {
        self.parameters.push(param);
        self
    }
}

/// Screen layout template mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutMode {
    /// Legacy NKS1 uniform 8-column layout.
    #[serde(rename = "nks1_layout")]
    Nks1Uniform,
    /// Next-gen widescreen NKS2 layout with semantic sections and cards.
    #[serde(rename = "nks2_layout")]
    Nks2Grouped,
    /// High-contrast live performance view.
    #[serde(rename = "performance_mode")]
    Performance,
}

/// View address used to target specific sections or groups for hardware transitions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_index: Option<u32>,
}

/// Binary graphic asset definition uploaded into the keyboard's on-device flash cache.
#[derive(Debug, Clone, PartialEq)]
pub struct FileAsset {
    /// Unique cache identifier (e.g. `"encdr_live_header"`, `"synth_logo"`).
    pub identifier: String,
    /// Raw compressed image bytes (PNG or JPEG).
    pub data: Vec<u8>,
}

impl FileAsset {
    pub fn new(identifier: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            identifier: identifier.into(),
            data,
        }
    }
}

// ── Plugin Chain Models ──────────────────────────────────────────────────────

/// Single plugin insert within a track's serial FX / instrument chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginChainItem {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    pub color: RgbColor,
    #[serde(default)]
    pub is_bypassed: bool,
}

impl PluginChainItem {
    pub fn new(name: impl Into<String>, color: RgbColor) -> Self {
        Self {
            name: name.into(),
            vendor: None,
            color,
            is_bypassed: false,
        }
    }

    pub fn with_vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into());
        self
    }

    pub fn bypassed(mut self, bypassed: bool) -> Self {
        self.is_bypassed = bypassed;
        self
    }
}

/// Serial plugin chain for the active track.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginChainModel {
    pub plugins: Vec<PluginChainItem>,
    pub current_index: u32,
}

impl PluginChainModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_plugin(&mut self, item: PluginChainItem) -> &mut Self {
        self.plugins.push(item);
        self
    }

    pub fn set_current_index(&mut self, index: u32) -> &mut Self {
        self.current_index = index;
        self
    }
}

// ── ODR Mixer Models ─────────────────────────────────────────────────────────

/// Individual track definition in the ODR mixer view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerTrack {
    pub name: String,
    pub color: RgbColor,
    pub volume: f32,
    pub pan: f32,
    pub volume_display: String,
    pub pan_display: String,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub soloed: bool,
    #[serde(default)]
    pub armed: bool,
    #[serde(default)]
    pub selected: bool,
}

impl MixerTrack {
    pub fn new(name: impl Into<String>, color: RgbColor) -> Self {
        Self {
            name: name.into(),
            color,
            volume: 0.8,
            pan: 0.0,
            volume_display: "0.0 dB".into(),
            pan_display: "C".into(),
            muted: false,
            soloed: false,
            armed: false,
            selected: false,
        }
    }
}

/// ODR Mixer state model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MixerModel {
    pub tracks: Vec<MixerTrack>,
    pub selected_index: u32,
}

impl MixerModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_track(&mut self, track: MixerTrack) -> &mut Self {
        self.tracks.push(track);
        self
    }
}

// ── Smart Play Models ────────────────────────────────────────────────────────

/// Scale engine configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaleConfig {
    pub enabled: bool,
    pub root_key: u8,
    pub scale_type: String,
    pub scale_mode: String,
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            root_key: 0,
            scale_type: "Major".into(),
            scale_mode: "Guide".into(),
        }
    }
}

/// Arpeggiator engine configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArpConfig {
    pub enabled: bool,
    pub pattern: String,
    pub rate: String,
    pub gate: f32,
    pub octaves: u8,
    pub swing: f32,
}

impl Default for ArpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pattern: "Up".into(),
            rate: "1/16".into(),
            gate: 0.75,
            octaves: 1,
            swing: 0.0,
        }
    }
}

/// Chord engine configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordConfig {
    pub enabled: bool,
    pub chord_mode: String,
    pub chord_type: String,
}

impl Default for ChordConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            chord_mode: "Off".into(),
            chord_type: "Octave".into(),
        }
    }
}

/// Comprehensive on-device Smart Play state (Scales, Chords, Arpeggiator).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SmartPlayData {
    pub scale: ScaleConfig,
    pub arp: ArpConfig,
    pub chord: ChordConfig,
}

// ── Sound & Preset Browser Models ────────────────────────────────────────────

/// Single filter category (column) in the on-device preset browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserFilter {
    pub name: String,
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_option: Option<String>,
}

impl BrowserFilter {
    pub fn new(name: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            name: name.into(),
            options,
            selected_option: None,
        }
    }

    pub fn with_selection(mut self, option: impl Into<String>) -> Self {
        self.selected_option = Some(option.into());
        self
    }
}

/// Sound / preset item in the on-device browser list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserSoundItem {
    pub name: String,
    pub vendor: String,
    pub product: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

impl BrowserSoundItem {
    pub fn new(name: impl Into<String>, vendor: impl Into<String>, product: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            vendor: vendor.into(),
            product: product.into(),
            author: None,
        }
    }
}

/// Preset / Sound Browser model displayed when pressing the BROWSER button.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BrowserModel {
    pub filters: Vec<BrowserFilter>,
    pub sounds: Vec<BrowserSoundItem>,
    pub selected_sound_index: Option<u32>,
}

// ── Device Settings & Standby ────────────────────────────────────────────────

/// Hardware device settings and preferences (brightness, velocity, standby).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub display_brightness: u8,
    pub led_brightness: u8,
    pub lightguide_enabled: bool,
    pub velocity_curve: String,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            display_brightness: 80,
            led_brightness: 80,
            lightguide_enabled: true,
            velocity_curve: "Linear".into(),
        }
    }
}

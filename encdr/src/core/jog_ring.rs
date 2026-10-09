use crate::core::led::LedValue;

/// Deck identifier for Traktor jog wheels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JogDeck {
    Left = 0,
    Right = 1,
}

impl JogDeck {
    pub fn index(self) -> u8 {
        self as u8
    }
}

/// S4 Mk3 jog wheel LED ring operating modes (Report 0x32, byte 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum JogRingMode {
    /// Ring LEDs turned off.
    Off = 0,
    /// Dim pulsing/flashing ring.
    DimFlash = 1,
    /// Illuminated needle spot indicator positioned by 16-bit tick offset (0..2879).
    Needle = 2,
    /// Full ring flash with base color.
    RingFlash = 3,
    /// Dim spot indicator positioned by 16-bit tick offset (0..2879).
    DimSpot = 4,
    /// Individually addressable 32-segment ring (bytes 8..39 in report).
    Addressable = 5,
}

impl JogRingMode {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Helper struct for constructing and manipulating a 32-segment addressable jog wheel LED ring buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JogRing {
    leds: [u8; 32],
}

impl Default for JogRing {
    fn default() -> Self {
        Self::new()
    }
}

impl JogRing {
    pub const SEGMENTS: usize = 32;
    pub const TICKS_PER_REV: u16 = 2880;

    /// Creates an empty (all off) jog ring.
    pub const fn new() -> Self {
        Self { leds: [0; 32] }
    }

    /// Creates a ring with all 32 segments set to a specific brightness or palette byte.
    pub const fn solid(value: u8) -> Self {
        Self {
            leds: [value; 32],
        }
    }

    /// Sets a single segment by index (0..31, wrapping if out of bounds).
    pub fn set(&mut self, index: usize, value: u8) {
        self.leds[index % Self::SEGMENTS] = value;
    }

    /// Sets a single segment by index using an `LedValue` (converting RGB to NI packed palette byte).
    pub fn set_led(&mut self, index: usize, value: LedValue) {
        let b = match value {
            LedValue::Off => 0,
            LedValue::Dim => LedValue::NI_DIM,
            LedValue::Bright => LedValue::NI_BRIGHT,
            LedValue::Single(v) => v,
            LedValue::Rgb { r, g, b } => LedValue::to_ni_palette_byte(r, g, b),
        };
        self.set(index, b);
    }

    /// Returns a reference to the raw 32-byte array.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.leds
    }

    /// Returns a mutable reference to the raw 32-byte array.
    pub fn as_bytes_mut(&mut self) -> &mut [u8; 32] {
        &mut self.leds
    }

    /// Creates a rotating spinner with a bright head and fading tail.
    ///
    /// - `head_index`: Head position (0..31).
    /// - `tail_length`: Number of trailing segments (1..32).
    /// - `head_value`: Brightness / palette byte for the head segment.
    pub fn spinner(head_index: usize, tail_length: usize, head_value: u8) -> Self {
        let mut ring = Self::new();
        let tail_len = tail_length.max(1).min(Self::SEGMENTS);
        for i in 0..tail_len {
            let idx = (head_index + Self::SEGMENTS - i) % Self::SEGMENTS;
            let factor = 1.0 - (i as f32 / tail_len as f32);
            let val = ((head_value as f32) * factor).round() as u8;
            ring.set(idx, val);
        }
        ring
    }

    /// Creates a meter / arc fill from 0 to `fill_count` segments (0..=32).
    pub fn meter(fill_count: usize, value: u8) -> Self {
        let mut ring = Self::new();
        let count = fill_count.min(Self::SEGMENTS);
        for i in 0..count {
            ring.set(i, value);
        }
        ring
    }

    /// Converts a continuous angle in radians (0.0 .. 2*PI) or jog ticks (0..2880) to a segment index (0..31).
    pub fn ticks_to_segment(ticks: u16) -> usize {
        let normalized = (ticks % Self::TICKS_PER_REV) as f32 / Self::TICKS_PER_REV as f32;
        ((normalized * Self::SEGMENTS as f32) as usize) % Self::SEGMENTS
    }
}

/// Helper struct for tracking a jog wheel's angular position across manual or motorized movements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JogWheelTracker {
    ticks: f32,
}

impl Default for JogWheelTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl JogWheelTracker {
    pub const fn new() -> Self {
        Self { ticks: 0.0 }
    }

    /// Creates a tracker with an initial tick position (0..2879).
    pub fn from_ticks(ticks: u16) -> Self {
        let mut tracker = Self::new();
        tracker.set_position(ticks);
        tracker
    }

    /// Sets the absolute position in ticks (0..2879).
    pub fn set_position(&mut self, ticks: u16) {
        self.ticks = (ticks % JogRing::TICKS_PER_REV) as f32;
    }

    /// Sets the position from a normalized rotation value (0.0..1.0).
    pub fn set_normalized(&mut self, normalized: f32) {
        self.ticks = (normalized.rem_euclid(1.0) * JogRing::TICKS_PER_REV as f32).rem_euclid(JogRing::TICKS_PER_REV as f32);
    }

    /// Sets the position from an angle in radians (0.0 .. 2*PI).
    pub fn set_radians(&mut self, radians: f32) {
        let normalized = radians / (2.0 * std::f32::consts::PI);
        self.set_normalized(normalized);
    }

    /// Advances the position by an encoder delta or motor displacement.
    ///
    /// - `delta`: Delta amount from an encoder event or motor step.
    /// - `ticks_per_unit`: Scale factor converting delta to 0..2879 ticks.
    pub fn advance(&mut self, delta: f32, ticks_per_unit: f32) {
        self.ticks = (self.ticks + delta * ticks_per_unit).rem_euclid(JogRing::TICKS_PER_REV as f32);
    }

    /// Returns the current absolute position in ticks (0..2879).
    pub fn current_ticks(&self) -> u16 {
        (self.ticks.round() as u16) % JogRing::TICKS_PER_REV
    }

    /// Returns the current position normalized to 0.0..1.0.
    pub fn current_normalized(&self) -> f32 {
        self.ticks / JogRing::TICKS_PER_REV as f32
    }

    /// Returns the corresponding 32-segment LED index (0..31).
    pub fn current_segment(&self) -> usize {
        JogRing::ticks_to_segment(self.current_ticks())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jog_deck_index() {
        assert_eq!(JogDeck::Left.index(), 0);
        assert_eq!(JogDeck::Right.index(), 1);
    }

    #[test]
    fn test_jog_ring_new_and_solid() {
        let ring = JogRing::new();
        assert_eq!(ring.as_bytes(), &[0u8; 32]);

        let solid = JogRing::solid(0x7F);
        assert_eq!(solid.as_bytes(), &[0x7F; 32]);
    }

    #[test]
    fn test_jog_ring_set_wrapping() {
        let mut ring = JogRing::new();
        ring.set(0, 42);
        ring.set(32, 99); // wraps to index 0
        assert_eq!(ring.as_bytes()[0], 99);
    }

    #[test]
    fn test_jog_ring_meter() {
        let m = JogRing::meter(4, 100);
        assert_eq!(&m.as_bytes()[0..4], &[100, 100, 100, 100]);
        assert_eq!(&m.as_bytes()[4..], &[0; 28]);
    }

    #[test]
    fn test_jog_ring_spinner() {
        let s = JogRing::spinner(10, 4, 100);
        assert!(s.as_bytes()[10] > 0);
        assert!(s.as_bytes()[9] > 0);
        assert!(s.as_bytes()[8] > 0);
        assert!(s.as_bytes()[7] > 0);
        assert_eq!(s.as_bytes()[6], 0);
    }

    #[test]
    fn test_ticks_to_segment() {
        assert_eq!(JogRing::ticks_to_segment(0), 0);
        assert_eq!(JogRing::ticks_to_segment(720), 8); // 1/4 revolution = 8 segments
        assert_eq!(JogRing::ticks_to_segment(1440), 16); // 1/2 revolution = 16 segments
        assert_eq!(JogRing::ticks_to_segment(2160), 24); // 3/4 revolution = 24 segments
    }

    #[test]
    fn test_jog_wheel_tracker() {
        let mut tracker = JogWheelTracker::new();
        assert_eq!(tracker.current_ticks(), 0);

        tracker.set_position(1440);
        assert_eq!(tracker.current_ticks(), 1440);
        assert_eq!(tracker.current_normalized(), 0.5);
        assert_eq!(tracker.current_segment(), 16);

        // Advance forward
        tracker.advance(7.2, 100.0); // +720 ticks
        assert_eq!(tracker.current_ticks(), 2160);
        assert_eq!(tracker.current_segment(), 24);

        // Advance wraparound
        tracker.advance(10.0, 100.0); // +1000 ticks -> 3160 % 2880 = 280
        assert_eq!(tracker.current_ticks(), 280);

        // Set normalized
        tracker.set_normalized(0.25);
        assert_eq!(tracker.current_ticks(), 720);
        assert_eq!(tracker.current_segment(), 8);
    }
}

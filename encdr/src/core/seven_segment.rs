/// 7-segment display helper for controllers with multi-segment numeric/alphanumeric displays
/// (e.g., Traktor Kontrol S4 MK2, Traktor Kontrol X1 MK2, Traktor Kontrol F1).
///
/// Segment Layout:
/// ```text
///       -- a (bit 0, 0x01) --
///      |                     |
///   f (bit 5, 0x20)       b (bit 1, 0x02)
///      |                     |
///       -- g (bit 6, 0x40) --
///      |                     |
///   e (bit 4, 0x10)       c (bit 2, 0x04)
///      |                     |
///       -- d (bit 3, 0x08) --     * dp (bit 7, 0x80)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SevenSegment {
    mask: u8,
}

impl SevenSegment {
    pub const SEG_A: u8 = 1 << 0;  // 0x01 (Top horizontal)
    pub const SEG_B: u8 = 1 << 1;  // 0x02 (Top right vertical)
    pub const SEG_C: u8 = 1 << 2;  // 0x04 (Bottom right vertical)
    pub const SEG_D: u8 = 1 << 3;  // 0x08 (Bottom horizontal)
    pub const SEG_E: u8 = 1 << 4;  // 0x10 (Bottom left vertical)
    pub const SEG_F: u8 = 1 << 5;  // 0x20 (Top left vertical)
    pub const SEG_G: u8 = 1 << 6;  // 0x40 (Middle horizontal)
    pub const SEG_DP: u8 = 1 << 7; // 0x80 (Decimal point)

    pub const BLANK: Self = Self { mask: 0 };

    /// Creates a 7-segment value from a raw 8-bit bitmask.
    pub const fn from_mask(mask: u8) -> Self {
        Self { mask }
    }

    /// Returns the raw 8-bit segment bitmask.
    pub const fn raw_mask(&self) -> u8 {
        self.mask
    }

    /// Toggles or sets the decimal point segment.
    pub const fn with_dot(mut self, dot: bool) -> Self {
        if dot {
            self.mask |= Self::SEG_DP;
        } else {
            self.mask &= !Self::SEG_DP;
        }
        self
    }

    /// Checks if the decimal point segment is lit.
    pub const fn has_dot(&self) -> bool {
        (self.mask & Self::SEG_DP) != 0
    }

    /// Converts a character into its standard 7-segment representation.
    pub fn from_char(c: char) -> Self {
        let mask = match c {
            '0' => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_F,
            '1' => Self::SEG_B | Self::SEG_C,
            '2' => Self::SEG_A | Self::SEG_B | Self::SEG_G | Self::SEG_E | Self::SEG_D,
            '3' => Self::SEG_A | Self::SEG_B | Self::SEG_G | Self::SEG_C | Self::SEG_D,
            '4' => Self::SEG_F | Self::SEG_G | Self::SEG_B | Self::SEG_C,
            '5' => Self::SEG_A | Self::SEG_F | Self::SEG_G | Self::SEG_C | Self::SEG_D,
            '6' => Self::SEG_A | Self::SEG_F | Self::SEG_E | Self::SEG_D | Self::SEG_C | Self::SEG_G,
            '7' => Self::SEG_A | Self::SEG_B | Self::SEG_C,
            '8' => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            '9' => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_F | Self::SEG_G,

            'a' | 'A' => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'b' | 'B' => Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'c'       => Self::SEG_D | Self::SEG_E | Self::SEG_G,
            'C'       => Self::SEG_A | Self::SEG_D | Self::SEG_E | Self::SEG_F,
            'd' | 'D' => Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_G,
            'e' | 'E' => Self::SEG_A | Self::SEG_D | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'f' | 'F' => Self::SEG_A | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'g' | 'G' => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_F | Self::SEG_G,
            'h'       => Self::SEG_C | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'H'       => Self::SEG_B | Self::SEG_C | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'i' | 'I' => Self::SEG_B | Self::SEG_C,
            'j' | 'J' => Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_E,
            'l' | 'L' => Self::SEG_D | Self::SEG_E | Self::SEG_F,
            'n' | 'N' => Self::SEG_C | Self::SEG_E | Self::SEG_G,
            'o'       => Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_G,
            'O'       => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_F,
            'p' | 'P' => Self::SEG_A | Self::SEG_B | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'q' | 'Q' => Self::SEG_A | Self::SEG_B | Self::SEG_C | Self::SEG_F | Self::SEG_G,
            'r' | 'R' => Self::SEG_E | Self::SEG_G,
            's' | 'S' => Self::SEG_A | Self::SEG_F | Self::SEG_G | Self::SEG_C | Self::SEG_D,
            't' | 'T' => Self::SEG_D | Self::SEG_E | Self::SEG_F | Self::SEG_G,
            'u'       => Self::SEG_C | Self::SEG_D | Self::SEG_E,
            'U'       => Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_E | Self::SEG_F,
            'y' | 'Y' => Self::SEG_B | Self::SEG_C | Self::SEG_D | Self::SEG_F | Self::SEG_G,

            '-' => Self::SEG_G,
            '_' => Self::SEG_D,
            '=' => Self::SEG_D | Self::SEG_G,
            '/' => Self::SEG_B | Self::SEG_G | Self::SEG_E,
            '.' => Self::SEG_DP,
            ' ' => 0,
            _ => 0,
        };
        Self { mask }
    }

    /// Converts a numeric digit (0..=15) into a 7-segment digit (hexadecimal).
    pub fn from_digit(digit: u8) -> Self {
        match digit {
            0..=9 => Self::from_char((b'0' + digit) as char),
            10..=15 => Self::from_char((b'A' + (digit - 10)) as char),
            _ => Self::BLANK,
        }
    }

    /// Returns an 8-byte array of segment brightnesses `[a, b, c, d, e, f, g, dp]`.
    ///
    /// Useful for hardware endpoints where each segment is addressed as an individual byte
    /// or strip item.
    pub fn to_brightness_array(&self, on_brightness: u8, off_brightness: u8) -> [u8; 8] {
        [
            if self.mask & Self::SEG_A != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_B != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_C != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_D != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_E != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_F != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_G != 0 { on_brightness } else { off_brightness },
            if self.mask & Self::SEG_DP != 0 { on_brightness } else { off_brightness },
        ]
    }

    /// Encodes a string into an array of SevenSegment digits.
    ///
    /// A period (`'.'`) automatically lights the decimal point of the preceding character.
    /// For example: `"1.2"` produces two digits (`'1'` with DP, followed by `'2'`).
    pub fn encode_str(s: &str) -> Vec<Self> {
        let mut digits: Vec<Self> = Vec::new();
        for ch in s.chars() {
            if ch == '.' {
                if let Some(last) = digits.last_mut() {
                    *last = last.with_dot(true);
                    continue;
                }
            }
            digits.push(Self::from_char(ch));
        }
        digits
    }

    /// Encodes a DJ loop length into a 2-digit 7-segment display.
    ///
    /// Follows Traktor Kontrol S4 notation: whole beat loops display the beat count directly,
    /// while fractional beat loops run the counter in reverse with the decimal dot illuminated:
    /// - `32.0`   -> `["3", "2"]`
    /// - `16.0`   -> `["1", "6"]`
    /// - `8.0`    -> `[" ", "8"]`
    /// - `4.0`    -> `[" ", "4"]`
    /// - `2.0`    -> `[" ", "2"]`
    /// - `1.0`    -> `[" ", "1"]`
    /// - `0.5`    -> `[".", "2"]` (renders `.2`)
    /// - `0.25`   -> `[".", "4"]` (renders `.4`)
    /// - `0.125`  -> `[".", "8"]` (renders `.8`)
    /// - `0.0625` -> `["1.", "6"]` (renders `1.6`, i.e. 1/16)
    /// - `0.03125`-> `["3.", "2"]` (renders `3.2`, i.e. 1/32)
    ///
    /// If `loop_active` is true, the decimal point on the right digit is lit to denote an active loop.
    pub fn encode_loop_length(beats: f32, loop_active: bool) -> [Self; 2] {
        let (d1, d2, d1_dot) = if beats >= 32.0 {
            ('3', '2', false)
        } else if beats >= 16.0 {
            ('1', '6', false)
        } else if beats >= 8.0 {
            (' ', '8', false)
        } else if beats >= 4.0 {
            (' ', '4', false)
        } else if beats >= 2.0 {
            (' ', '2', false)
        } else if beats >= 1.0 {
            (' ', '1', false)
        } else if beats >= 0.5 {
            (' ', '2', true) // .2
        } else if beats >= 0.25 {
            (' ', '4', true) // .4
        } else if beats >= 0.125 {
            (' ', '8', true) // .8
        } else if beats >= 0.0625 {
            ('1', '6', true) // 1.6
        } else if beats >= 0.03125 {
            ('3', '2', true) // 3.2
        } else {
            ('-', '-', false)
        };

        let left = Self::from_char(d1).with_dot(d1_dot);
        let right = Self::from_char(d2).with_dot(loop_active);
        [left, right]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digits() {
        assert_eq!(SevenSegment::from_char('0').raw_mask(), 0x3F);
        assert_eq!(SevenSegment::from_char('1').raw_mask(), 0x06);
        assert_eq!(SevenSegment::from_char('8').raw_mask(), 0x7F);
    }

    #[test]
    fn test_with_dot() {
        let seg = SevenSegment::from_char('4').with_dot(true);
        assert!(seg.has_dot());
        assert_eq!(seg.raw_mask() & SevenSegment::SEG_DP, SevenSegment::SEG_DP);
    }

    #[test]
    fn test_encode_str_with_period() {
        let encoded = SevenSegment::encode_str("1.6");
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0], SevenSegment::from_char('1').with_dot(true));
        assert_eq!(encoded[1], SevenSegment::from_char('6'));
    }

    #[test]
    fn test_loop_lengths() {
        let loop_32 = SevenSegment::encode_loop_length(32.0, false);
        assert_eq!(loop_32[0], SevenSegment::from_char('3'));
        assert_eq!(loop_32[1], SevenSegment::from_char('2'));

        let loop_quarter = SevenSegment::encode_loop_length(0.25, false);
        assert_eq!(loop_quarter[0], SevenSegment::from_char('.'));
        assert_eq!(loop_quarter[1], SevenSegment::from_char('4'));

        let loop_half_active = SevenSegment::encode_loop_length(0.5, true);
        assert_eq!(loop_half_active[0], SevenSegment::from_char('.'));
        assert_eq!(loop_half_active[1], SevenSegment::from_char('2').with_dot(true));
    }

    #[test]
    fn test_brightness_array() {
        let seg = SevenSegment::from_char('1'); // B and C
        let arr = seg.to_brightness_array(255, 0);
        assert_eq!(arr, [0, 255, 255, 0, 0, 0, 0, 0]);
    }
}

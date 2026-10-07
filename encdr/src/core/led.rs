/// Value to set on an LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedValue {
    Off,
    /// Dim / half-brightness level (maps to NI 30% duty cycle / 0x9E on monochrome LEDs).
    Dim,
    /// Bright / active level (maps to NI 100% duty cycle / 0xE4 on monochrome LEDs).
    Bright,
    Single(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl Default for LedValue {
    fn default() -> Self {
        LedValue::Off
    }
}

impl LedValue {
    /// Native Instruments standard OFF state (`0x00`).
    pub const OFF: LedValue = LedValue::Off;

    /// Native Instruments standard DIM / half-brightness level (`0x9E` / 158: 30% duty cycle).
    pub const DIM: LedValue = LedValue::Dim;

    /// Native Instruments standard BRIGHT / active level (`0xE4` / 228: 100% duty cycle).
    pub const BRIGHT: LedValue = LedValue::Bright;

    /// Native Instruments maximum drive level (`0xFF` / 255: max drive / intensity 3).
    pub const MAX: LedValue = LedValue::Single(255);

    /// Raw byte for NI single-color button DIM state (`0x9E` / 158).
    pub const NI_DIM: u8 = 158;

    /// Raw byte for NI single-color button BRIGHT state (`0xE4` / 228).
    pub const NI_BRIGHT: u8 = 228;

    /// Raw byte for NI single-color button MAX state (`0xFF` / 255).
    pub const NI_MAX: u8 = 255;

    /// Raw byte for NI single-color button OFF state (`0x00`).
    pub const NI_OFF: u8 = 0;

    pub fn brightness(&self) -> u8 {
        match self {
            LedValue::Off => 0,
            LedValue::Dim => Self::NI_DIM,
            LedValue::Bright => Self::NI_BRIGHT,
            LedValue::Single(b) => *b,
            LedValue::Rgb { r, g, b } => (*r).max(*g).max(*b),
        }
    }

    /// Converts a brightness percentage (0..=100) to Native Instruments 7-bit PWM byte format
    /// used by single-color button LEDs.
    ///
    /// - `0`: Off (`0x00`)
    /// - `1..=100`: `0x80 | pct` (e.g. 30% -> `0x9E` / 158, 100% -> `0xE4` / 228)
    pub fn to_ni_single_byte(pct: u8) -> u8 {
        if pct == 0 {
            0
        } else {
            0x80 | pct.min(100)
        }
    }

    /// Convenience constructor for an NI single-color button set to a specific percentage (0..=100).
    pub fn single_percent(pct: u8) -> Self {
        if pct == 0 {
            LedValue::Off
        } else {
            LedValue::Single(Self::to_ni_single_byte(pct))
        }
    }

    /// Converts an RGB color to the Native Instruments packed 1-byte palette format
    /// used by the Maschine Mk3, Maschine Mikro Mk3, and Komplete Kontrol Mk2 series.
    ///
    /// The NI packed format:
    /// - `0x00`: LED Off
    /// - Bits 7..2: Palette color ID (1..17, where 1=Red ... 16=Crimson, 17=White)
    /// - Bits 1..0: 2-bit intensity level (0..3)
    pub fn to_ni_palette_byte(r: u8, g: u8, b: u8) -> u8 {
        let max_val = r.max(g).max(b);
        if max_val == 0 {
            return 0;
        }

        let intensity = match max_val {
            0..=31 => 0,
            32..=95 => 1,
            96..=191 => 2,
            _ => 3,
        };

        // Standard 17-color Native Instruments hardware palette
        const NI_PALETTE: [(i32, i32, i32); 17] = [
            (255, 0, 0),     // 0: Red
            (255, 63, 0),    // 1: Orange-Red
            (255, 127, 0),   // 2: Orange
            (255, 207, 0),   // 3: Warm Yellow / Amber
            (255, 255, 0),   // 4: Yellow
            (127, 255, 0),   // 5: Lime
            (0, 255, 0),     // 6: Green
            (0, 255, 127),   // 7: Mint
            (0, 255, 255),   // 8: Cyan
            (0, 127, 255),   // 9: Sky Blue
            (0, 0, 255),     // 10: Blue
            (63, 0, 255),    // 11: Indigo
            (127, 0, 255),   // 12: Violet / Purple
            (255, 0, 255),   // 13: Magenta / Pink
            (255, 0, 127),   // 14: Rose
            (255, 0, 63),    // 15: Crimson
            (255, 255, 255), // 16: White
        ];

        let nr = (r as i32 * 255) / max_val as i32;
        let ng = (g as i32 * 255) / max_val as i32;
        let nb = (b as i32 * 255) / max_val as i32;

        let mut best_idx = 0;
        let mut best_dist = i32::MAX;

        for (idx, &(pr, pg, pb)) in NI_PALETTE.iter().enumerate() {
            let dr = nr - pr;
            let dg = ng - pg;
            let db = nb - pb;
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best_idx = idx;
            }
        }

        let color_num = (best_idx + 1) as u8;
        (color_num << 2) | intensity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ni_palette_off() {
        assert_eq!(LedValue::to_ni_palette_byte(0, 0, 0), 0x00);
    }

    #[test]
    fn test_ni_palette_red() {
        // Red is color 1. High brightness -> intensity 3. (1 << 2) | 3 = 7 (0x07)
        let b = LedValue::to_ni_palette_byte(255, 0, 0);
        assert_eq!(b, 0x07);
    }

    #[test]
    fn test_ni_palette_white() {
        // White is color 17. High brightness -> intensity 3. (17 << 2) | 3 = 71 (0x47)
        let b = LedValue::to_ni_palette_byte(255, 255, 255);
        assert_eq!(b, 0x47);
    }

    #[test]
    fn test_ni_palette_blue() {
        // Blue is color 11 (index 10). (11 << 2) | 3 = 47 (0x2f)
        let b = LedValue::to_ni_palette_byte(0, 0, 255);
        assert_eq!(b, 0x2f);
    }

    #[test]
    fn test_ni_single_levels() {
        assert_eq!(LedValue::to_ni_single_byte(0), 0);
        assert_eq!(LedValue::to_ni_single_byte(30), 158); // 0x9E (Dim)
        assert_eq!(LedValue::to_ni_single_byte(100), 228); // 0xE4 (Bright)
        assert_eq!(LedValue::to_ni_single_byte(120), 228); // Clamped to 100
        assert_eq!(LedValue::single_percent(0), LedValue::Off);
        assert_eq!(LedValue::single_percent(30), LedValue::Single(158));
        assert_eq!(LedValue::single_percent(100), LedValue::Single(228));
    }

    #[test]
    fn test_led_value_brightness() {
        assert_eq!(LedValue::Off.brightness(), 0);
        assert_eq!(LedValue::Dim.brightness(), 158);
        assert_eq!(LedValue::Bright.brightness(), 228);
        assert_eq!(LedValue::Single(42).brightness(), 42);
        assert_eq!(LedValue::Rgb { r: 10, g: 90, b: 30 }.brightness(), 90);
    }
}



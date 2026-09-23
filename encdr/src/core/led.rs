/// Value to set on an LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedValue {
    Off,
    Single(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl Default for LedValue {
    fn default() -> Self {
        LedValue::Off
    }
}

impl LedValue {
    pub fn brightness(&self) -> u8 {
        match self {
            LedValue::Off => 0,
            LedValue::Single(b) => *b,
            LedValue::Rgb { r, g, b } => (*r).max(*g).max(*b),
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
}



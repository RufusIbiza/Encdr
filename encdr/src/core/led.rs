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
}

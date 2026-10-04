/// Tracks encoder state for delta calculation across USB packets.
#[derive(Debug)]
pub struct EncoderState {
    /// Current raw value (for wrap-around encoders)
    pub value: u16,
    /// Whether we've received at least one packet (skip first delta)
    pub initialized: bool,
}

impl Default for EncoderState {
    fn default() -> Self {
        Self {
            value: 0,
            initialized: false,
        }
    }
}

impl EncoderState {
    /// Update with a new raw value and return the delta.
    /// Returns None on the first packet (no previous value to compare).
    pub fn update_wrap16(&mut self, new_val: u8) -> Option<i32> {
        let new_val = new_val & 0x0f; // 4-bit value
        if !self.initialized {
            self.value = new_val as u16;
            self.initialized = true;
            return None;
        }
        let prev = self.value as u8;
        if new_val == prev {
            return None;
        }
        let delta = wrap_delta_4bit(new_val, prev);
        self.value = new_val as u16;
        Some(delta)
    }

    /// Update with a new 16-bit signed value and return the delta.
    pub fn update_signed16(&mut self, new_val: u16) -> Option<f32> {
        if !self.initialized {
            self.value = new_val;
            self.initialized = true;
            return None;
        }
        if new_val == self.value {
            return None;
        }
        let delta = new_val as f32 - self.value as f32;
        self.value = new_val;
        Some(delta)
    }

    /// Update with a new 16-bit absolute position with full wraparound.
    /// Used for jogwheels/jogdials that report a continuous 16-bit counter.
    /// Returns the signed delta via shortest path around the 65536-step ring.
    pub fn update_wrap16_wide(&mut self, new_val: u16) -> Option<i32> {
        if !self.initialized {
            self.value = new_val;
            self.initialized = true;
            return None;
        }
        if new_val == self.value {
            return None;
        }
        let delta = wrap_delta_16bit(new_val, self.value);
        self.value = new_val;
        Some(delta)
    }

    /// Update with a new absolute ERP position (0..999) and return the delta
    /// via shortest path around the 1000-step ring. Movements smaller than
    /// `deadband` are held back (not lost) until they accumulate past it, to
    /// suppress analog wobble.
    pub fn update_erp(&mut self, pos: u16, deadband: u16) -> Option<i32> {
        if !self.initialized {
            self.value = pos;
            self.initialized = true;
            return None;
        }
        let delta = wrap_delta_erp(pos, self.value);
        if delta == 0 || delta.unsigned_abs() < deadband as u32 {
            return None;
        }
        self.value = pos;
        Some(delta)
    }

    /// Update with a new raw value for a slider, return value if changed.
    pub fn update_slider(&mut self, new_val: u16) -> Option<u16> {
        if !self.initialized {
            self.value = new_val;
            self.initialized = true;
            return Some(new_val);
        }
        if new_val == self.value {
            return None;
        }
        self.value = new_val;
        Some(new_val)
    }
}

/// 4-bit counter wrap-around delta detection (used by D2 browse/loop encoders).
/// Detects direction via shortest path around the 16-step ring.
#[inline]
fn wrap_delta_4bit(now: u8, prev: u8) -> i32 {
    let diff = (now as i32) - (prev as i32);
    if diff > 7 {
        diff - 16
    } else if diff < -7 {
        diff + 16
    } else {
        diff
    }
}

/// 16-bit counter wrap-around delta detection (used by jogwheels/jogdials).
/// Detects direction via shortest path around the 65536-step ring.
#[inline]
fn wrap_delta_16bit(now: u16, prev: u16) -> i32 {
    let diff = (now as i32) - (prev as i32);
    if diff > 32767 {
        diff - 65536
    } else if diff < -32767 {
        diff + 65536
    } else {
        diff
    }
}

/// ERP position wrap-around delta. Detects direction via shortest path
/// around the 1000-step ring.
#[inline]
fn wrap_delta_erp(now: u16, prev: u16) -> i32 {
    (now as i32 - prev as i32 + 1500).rem_euclid(1000) - 500
}

/// Decode an endless rotary potentiometer (ERP) to an absolute position
/// 0..999. `a` and `b` are the two analog wiper taps, 90° apart; each is
/// linear over half a turn, so the result blends both by how close `a` is
/// to its extremes. Port of `decode_erp` from the Linux snd-usb-caiaq
/// driver (sound/usb/caiaq/input.c), which uses the same calibration
/// constants for every NI ERP device.
pub fn decode_erp(a: u8, b: u8) -> u16 {
    const HIGH_PEAK: i32 = 268;
    const LOW_PEAK: i32 = -7;
    const RANGE: i32 = HIGH_PEAK - LOW_PEAK;
    const MID: i32 = (HIGH_PEAK + LOW_PEAK) / 2;
    const DEG90: i32 = RANGE / 2;
    const DEG180: i32 = RANGE;
    const DEG270: i32 = DEG90 + DEG180;
    const DEG360: i32 = DEG180 * 2;

    let (a, b) = (a as i32, b as i32);
    let weight_b = ((MID - a).abs() - (RANGE / 2 - 100) / 2).clamp(0, 100);
    let weight_a = 100 - weight_b;

    let pos_b = if a < MID {
        // 0..90 and 270..360 degrees
        let p = b - LOW_PEAK + DEG270;
        if p >= DEG360 { p - DEG360 } else { p }
    } else {
        // 90..270 degrees
        HIGH_PEAK - b + DEG90
    };
    let pos_a = if b > MID {
        // 0..180 degrees
        a - LOW_PEAK
    } else {
        // 180..360 degrees
        HIGH_PEAK - a + DEG180
    };

    let ret = (pos_a * weight_a + pos_b * weight_b) * 10 / DEG360;
    ret.rem_euclid(1000) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_erp_matches_hardware_capture() {
        // Maschine Mk1 0x02 report captured at rest; pair i is (b, a) at
        // bytes [1 + 2i, 2 + 2i]. Expected values from the kernel decoder.
        let pkt = [
            0x02, 0xa0, 0x15, 0xb7, 0xcc, 0x03, 0x71, 0x85, 0x03, 0x1f, 0xa2, 0x85,
            0x00, 0x3b, 0x3d, 0x5d, 0x17, 0x19, 0x5f, 0x0c, 0x86, 0xf7, 0x65,
        ];
        let decoded: Vec<u16> = (0..11).map(|i| decode_erp(pkt[2 + 2 * i], pkt[1 + 2 * i])).collect();
        assert_eq!(decoded, vec![52, 394, 781, 3, 690, 3, 872, 932, 813, 743, 197]);
    }

    #[test]
    fn erp_wraps_forward_and_backward() {
        let mut enc = EncoderState::default();
        assert_eq!(enc.update_erp(990, 0), None);
        assert_eq!(enc.update_erp(7, 0), Some(17));
        assert_eq!(enc.update_erp(995, 0), Some(-12));
    }

    #[test]
    fn erp_deadband_accumulates() {
        let mut enc = EncoderState::default();
        enc.update_erp(100, 5);
        assert_eq!(enc.update_erp(103, 5), None);
        assert_eq!(enc.update_erp(101, 5), None);
        assert_eq!(enc.update_erp(106, 5), Some(6));
    }

    #[test]
    fn wrap16_forward() {
        let mut enc = EncoderState::default();
        assert_eq!(enc.update_wrap16(5), None); // first packet
        assert_eq!(enc.update_wrap16(6), Some(1));
        assert_eq!(enc.update_wrap16(7), Some(1));
    }

    #[test]
    fn wrap16_backward() {
        let mut enc = EncoderState::default();
        enc.update_wrap16(3);
        assert_eq!(enc.update_wrap16(2), Some(-1));
        assert_eq!(enc.update_wrap16(1), Some(-1));
    }

    #[test]
    fn wrap16_wraparound_forward() {
        let mut enc = EncoderState::default();
        enc.update_wrap16(14);
        assert_eq!(enc.update_wrap16(15), Some(1));
        assert_eq!(enc.update_wrap16(0), Some(1));
        assert_eq!(enc.update_wrap16(1), Some(1));
    }

    #[test]
    fn wrap16_wraparound_backward() {
        let mut enc = EncoderState::default();
        enc.update_wrap16(1);
        assert_eq!(enc.update_wrap16(0), Some(-1));
        assert_eq!(enc.update_wrap16(15), Some(-1));
        assert_eq!(enc.update_wrap16(14), Some(-1));
    }

    #[test]
    fn wrap16_no_change() {
        let mut enc = EncoderState::default();
        enc.update_wrap16(5);
        assert_eq!(enc.update_wrap16(5), None);
    }

    #[test]
    fn wrap16_wide_forward() {
        let mut enc = EncoderState::default();
        assert_eq!(enc.update_wrap16_wide(1000), None);
        assert_eq!(enc.update_wrap16_wide(1005), Some(5));
        assert_eq!(enc.update_wrap16_wide(1010), Some(5));
    }

    #[test]
    fn wrap16_wide_backward() {
        let mut enc = EncoderState::default();
        enc.update_wrap16_wide(1000);
        assert_eq!(enc.update_wrap16_wide(995), Some(-5));
    }

    #[test]
    fn wrap16_wide_wraparound_forward() {
        let mut enc = EncoderState::default();
        enc.update_wrap16_wide(65530);
        assert_eq!(enc.update_wrap16_wide(65535), Some(5));
        assert_eq!(enc.update_wrap16_wide(4), Some(5)); // 65535 → 4 = +5
    }

    #[test]
    fn wrap16_wide_wraparound_backward() {
        let mut enc = EncoderState::default();
        enc.update_wrap16_wide(5);
        assert_eq!(enc.update_wrap16_wide(0), Some(-5));
        assert_eq!(enc.update_wrap16_wide(65531), Some(-5)); // 0 → 65531 = -5
    }

    #[test]
    fn wrap16_wide_no_change() {
        let mut enc = EncoderState::default();
        enc.update_wrap16_wide(500);
        assert_eq!(enc.update_wrap16_wide(500), None);
    }
}

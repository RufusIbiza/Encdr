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

#[cfg(test)]
mod tests {
    use super::*;

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

use crate::RgbColor;

/// Number of integer RGB steps in one full red-green-blue spectrum cycle.
pub const SPECTRUM_STEPS: u16 = 6 * 256;

/// Return one saturated color from a deterministic RGB spectrum.
///
/// `phase` wraps after [`SPECTRUM_STEPS`], which keeps animation timing and
/// rendering independent from USB transport and device-specific code.
pub const fn spectrum_color(phase: u16) -> RgbColor {
    let phase = phase % SPECTRUM_STEPS;
    let segment = phase / 256;
    let offset = (phase % 256) as u8;
    let inverse = u8::MAX - offset;

    match segment {
        0 => RgbColor::new(u8::MAX, offset, 0),
        1 => RgbColor::new(inverse, u8::MAX, 0),
        2 => RgbColor::new(0, u8::MAX, offset),
        3 => RgbColor::new(0, inverse, u8::MAX),
        4 => RgbColor::new(offset, 0, u8::MAX),
        _ => RgbColor::new(u8::MAX, 0, inverse),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_has_stable_primary_and_secondary_anchors() {
        assert_eq!(spectrum_color(0), RgbColor::new(255, 0, 0));
        assert_eq!(spectrum_color(256), RgbColor::new(255, 255, 0));
        assert_eq!(spectrum_color(512), RgbColor::new(0, 255, 0));
        assert_eq!(spectrum_color(768), RgbColor::new(0, 255, 255));
        assert_eq!(spectrum_color(1024), RgbColor::new(0, 0, 255));
        assert_eq!(spectrum_color(1280), RgbColor::new(255, 0, 255));
        assert_eq!(spectrum_color(SPECTRUM_STEPS), spectrum_color(0));
    }
}

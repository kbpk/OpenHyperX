use crate::RgbColor;
use serde::{Deserialize, Serialize};

/// One DPI level exposed by a device profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DpiStage {
    pub x: u32,
    pub y: u32,
    pub color: RgbColor,
}

impl DpiStage {
    pub const fn new(x: u32, y: u32, color: RgbColor) -> Self {
        Self { x, y, color }
    }
}

/// Enabled DPI levels and the zero-based index of the active level.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DpiProfile {
    pub stages: Vec<DpiStage>,
    pub active_stage: usize,
}

impl DpiProfile {
    pub fn active(&self) -> Option<&DpiStage> {
        self.stages.get(self.active_stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_active_stage() {
        let profile = DpiProfile {
            stages: vec![
                DpiStage::new(800, 800, RgbColor::new(0, 0, 0xFF)),
                DpiStage::new(1600, 1600, RgbColor::new(0xFF, 0, 0)),
            ],
            active_stage: 1,
        };

        assert_eq!(
            profile.active(),
            Some(&DpiStage::new(1600, 1600, RgbColor::new(0xFF, 0, 0)))
        );
    }
}

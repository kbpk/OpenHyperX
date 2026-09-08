use std::str::FromStr;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbColor {
    pub const BLACK: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
    };

    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub const fn bytes(self) -> [u8; 3] {
        [self.red, self.green, self.blue]
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ColorParseError {
    #[error("RGB color must contain exactly six hexadecimal digits, for example FF8000")]
    InvalidLength,
    #[error("RGB color contains a non-hexadecimal digit")]
    InvalidHex,
}

impl FromStr for RgbColor {
    type Err = ColorParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let hex = input.strip_prefix('#').unwrap_or(input);
        if hex.len() != 6 {
            return Err(ColorParseError::InvalidLength);
        }

        let red = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError::InvalidHex)?;
        let green = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError::InvalidHex)?;
        let blue = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError::InvalidHex)?;
        Ok(Self::new(red, green, blue))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hash_and_plain_rgb() {
        assert_eq!("FF8000".parse(), Ok(RgbColor::new(0xFF, 0x80, 0x00)));
        assert_eq!("#00ff7F".parse(), Ok(RgbColor::new(0x00, 0xFF, 0x7F)));
    }

    #[test]
    fn rejects_wrong_length_and_non_hex_input() {
        assert_eq!(
            "FFF".parse::<RgbColor>(),
            Err(ColorParseError::InvalidLength)
        );
        assert_eq!(
            "GG0000".parse::<RgbColor>(),
            Err(ColorParseError::InvalidHex)
        );
    }
}

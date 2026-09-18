//! Vendor-protocol primitives shared by device drivers.
//!
//! Packet codecs in this crate do not perform I/O.

use std::fmt::Write;

pub mod capture;
pub mod hid_descriptor;
pub mod ngenuity_legacy;
pub mod pulsefire_raid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Tx,
    Rx,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tx => formatter.write_str("TX"),
            Self::Rx => formatter.write_str("RX"),
        }
    }
}

pub fn format_hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len().saturating_mul(3));
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            result.push(' ');
        }
        write!(&mut result, "{byte:02X}").expect("writing to a String cannot fail");
    }
    result
}

/// Log a raw report only when trace logging is enabled.
pub fn trace_report(direction: Direction, interface: i32, report: &[u8]) {
    let direction = match direction {
        Direction::Tx => "TX",
        Direction::Rx => "RX",
    };
    let report_id = report.first().copied().unwrap_or(0);

    tracing::trace!(
        target: "hyperx_protocol::wire",
        direction,
        interface,
        report_id = format_args!("0x{report_id:02X}"),
        bytes = %format_hex(report),
        "raw HID report"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_hex_format_is_uppercase_and_space_separated() {
        assert_eq!(format_hex(&[0x07, 0x0A, 0x00, 0xA0]), "07 0A 00 A0");
    }

    #[test]
    fn empty_report_formats_cleanly() {
        assert_eq!(format_hex(&[]), "");
    }
}

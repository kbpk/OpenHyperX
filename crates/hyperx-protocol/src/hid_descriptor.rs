use std::collections::BTreeMap;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReportLayout {
    pub report_id: u8,
    pub input_bits: u32,
    pub output_bits: u32,
    pub feature_bits: u32,
}

impl ReportLayout {
    pub fn input_bytes(self) -> u32 {
        bytes_for_bits(self.input_bits)
    }

    pub fn output_bytes(self) -> u32 {
        bytes_for_bits(self.output_bits)
    }

    pub fn feature_bytes(self) -> u32 {
        bytes_for_bits(self.feature_bits)
    }

    pub fn hidapi_feature_bytes(self) -> u32 {
        self.feature_bytes() + u32::from(self.report_id != 0)
    }
}

const fn bytes_for_bits(bits: u32) -> u32 {
    bits.saturating_add(7) / 8
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DescriptorError {
    #[error("truncated HID short item at byte {offset}")]
    TruncatedShortItem { offset: usize },
    #[error("truncated HID long item at byte {offset}")]
    TruncatedLongItem { offset: usize },
    #[error("HID global state stack underflow at byte {offset}")]
    GlobalStackUnderflow { offset: usize },
    #[error("HID report size multiplied by count overflows at byte {offset}")]
    ReportSizeOverflow { offset: usize },
}

#[derive(Clone, Copy, Debug, Default)]
struct Globals {
    report_size: u32,
    report_count: u32,
    report_id: u8,
}

pub fn parse_report_layouts(descriptor: &[u8]) -> Result<Vec<ReportLayout>, DescriptorError> {
    let mut layouts = BTreeMap::<u8, ReportLayout>::new();
    let mut globals = Globals::default();
    let mut stack = Vec::new();
    let mut offset = 0;

    while offset < descriptor.len() {
        let item_offset = offset;
        let prefix = descriptor[offset];
        offset += 1;

        if prefix == 0xFE {
            if offset + 2 > descriptor.len() {
                return Err(DescriptorError::TruncatedLongItem {
                    offset: item_offset,
                });
            }
            let length = usize::from(descriptor[offset]);
            offset += 2;
            if offset + length > descriptor.len() {
                return Err(DescriptorError::TruncatedLongItem {
                    offset: item_offset,
                });
            }
            offset += length;
            continue;
        }

        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        if offset + size > descriptor.len() {
            return Err(DescriptorError::TruncatedShortItem {
                offset: item_offset,
            });
        }
        let value = read_unsigned(&descriptor[offset..offset + size]);
        offset += size;

        let item_type = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0F;
        match (item_type, tag) {
            (1, 0x07) => globals.report_size = value,
            (1, 0x08) => globals.report_id = value as u8,
            (1, 0x09) => globals.report_count = value,
            (1, 0x0A) => stack.push(globals),
            (1, 0x0B) => {
                globals = stack.pop().ok_or(DescriptorError::GlobalStackUnderflow {
                    offset: item_offset,
                })?;
            }
            (0, 0x08 | 0x09 | 0x0B) => {
                let bits = globals
                    .report_size
                    .checked_mul(globals.report_count)
                    .ok_or(DescriptorError::ReportSizeOverflow {
                        offset: item_offset,
                    })?;
                let layout = layouts.entry(globals.report_id).or_insert(ReportLayout {
                    report_id: globals.report_id,
                    ..ReportLayout::default()
                });
                match tag {
                    0x08 => layout.input_bits = layout.input_bits.saturating_add(bits),
                    0x09 => layout.output_bits = layout.output_bits.saturating_add(bits),
                    0x0B => layout.feature_bits = layout.feature_bits.saturating_add(bits),
                    _ => unreachable!(),
                }
            }
            _ => {}
        }
    }

    Ok(layouts.into_values().collect())
}

fn read_unsigned(bytes: &[u8]) -> u32 {
    bytes.iter().enumerate().fold(0, |value, (shift, byte)| {
        value | (u32::from(*byte) << (shift * 8))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbered_feature_report_and_hidapi_length() {
        let descriptor = [
            0x85, 0x07, // Report ID 7
            0x75, 0x08, // Report Size 8 bits
            0x96, 0x07, 0x01, // Report Count 263
            0xB1, 0x02, // Feature
        ];

        let layouts = parse_report_layouts(&descriptor).unwrap();
        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].report_id, 7);
        assert_eq!(layouts[0].feature_bytes(), 263);
        assert_eq!(layouts[0].hidapi_feature_bytes(), 264);
    }

    #[test]
    fn accumulates_main_items_and_separates_report_ids() {
        let descriptor = [
            0x85, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, // ID 1, 8 input bits
            0x95, 0x10, 0x91, 0x02, // ID 1, 16 output bits
            0x85, 0x02, 0x75, 0x08, 0x95, 0x03, 0xB1, 0x02, // ID 2, 24 feature bits
        ];

        let layouts = parse_report_layouts(&descriptor).unwrap();
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].input_bytes(), 1);
        assert_eq!(layouts[0].output_bytes(), 2);
        assert_eq!(layouts[1].feature_bytes(), 3);
    }

    #[test]
    fn rejects_truncated_items() {
        assert_eq!(
            parse_report_layouts(&[0x76, 0x08]),
            Err(DescriptorError::TruncatedShortItem { offset: 0 })
        );
        assert_eq!(
            parse_report_layouts(&[0xFE, 0x04, 0x01, 0xAA]),
            Err(DescriptorError::TruncatedLongItem { offset: 0 })
        );
    }
}

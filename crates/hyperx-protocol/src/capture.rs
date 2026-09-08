use thiserror::Error;

use crate::Direction;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedReport {
    pub direction: Option<Direction>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteChange {
    pub offset: usize,
    pub before: Option<u8>,
    pub after: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportDiff {
    pub index: usize,
    pub before_direction: Option<Direction>,
    pub after_direction: Option<Direction>,
    pub before_len: usize,
    pub after_len: usize,
    pub changes: Vec<ByteChange>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CaptureParseError {
    #[error("line {line}: expected at least one hexadecimal byte")]
    EmptyReport { line: usize },
    #[error("line {line}: invalid byte `{token}`; use two hexadecimal digits")]
    InvalidByte { line: usize, token: String },
}

/// Parse one report per line. Lines may start with `TX` or `RX`; blank lines
/// and comments beginning with `#` are ignored.
pub fn parse_hex_capture(input: &str) -> Result<Vec<CapturedReport>, CaptureParseError> {
    let mut reports = Vec::new();
    for (line_index, original_line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        let line = original_line
            .split_once('#')
            .map_or(original_line, |(content, _)| content)
            .trim();
        if line.is_empty() {
            continue;
        }

        let mut tokens = line.split_whitespace();
        let first = tokens.next().expect("a non-empty line has a first token");
        let (direction, first_byte) = match first.to_ascii_uppercase().as_str() {
            "TX" => (Some(Direction::Tx), None),
            "RX" => (Some(Direction::Rx), None),
            _ => (None, Some(first)),
        };
        let byte_tokens = first_byte.into_iter().chain(tokens);
        let mut bytes = Vec::new();
        for token in byte_tokens {
            let hex = token
                .strip_prefix("0x")
                .or_else(|| token.strip_prefix("0X"))
                .unwrap_or(token);
            if hex.len() != 2 {
                return Err(CaptureParseError::InvalidByte {
                    line: line_number,
                    token: token.to_owned(),
                });
            }
            let byte = u8::from_str_radix(hex, 16).map_err(|_| CaptureParseError::InvalidByte {
                line: line_number,
                token: token.to_owned(),
            })?;
            bytes.push(byte);
        }
        if bytes.is_empty() {
            return Err(CaptureParseError::EmptyReport { line: line_number });
        }
        reports.push(CapturedReport { direction, bytes });
    }
    Ok(reports)
}

pub fn diff_captures(before: &[CapturedReport], after: &[CapturedReport]) -> Vec<ReportDiff> {
    let report_count = before.len().max(after.len());
    (0..report_count)
        .filter_map(|index| {
            let left = before.get(index);
            let right = after.get(index);
            let before_len = left.map_or(0, |report| report.bytes.len());
            let after_len = right.map_or(0, |report| report.bytes.len());
            let byte_count = before_len.max(after_len);
            let changes: Vec<_> = (0..byte_count)
                .filter_map(|offset| {
                    let before = left.and_then(|report| report.bytes.get(offset)).copied();
                    let after = right.and_then(|report| report.bytes.get(offset)).copied();
                    (before != after).then_some(ByteChange {
                        offset,
                        before,
                        after,
                    })
                })
                .collect();
            let before_direction = left.and_then(|report| report.direction);
            let after_direction = right.and_then(|report| report.direction);

            (!changes.is_empty()
                || before_direction != after_direction
                || left.is_none()
                || right.is_none())
            .then_some(ReportDiff {
                index,
                before_direction,
                after_direction,
                before_len,
                after_len,
                changes,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directions_comments_and_optional_prefixes() {
        let capture =
            parse_hex_capture("# test\nTX 07 0A FF 00\nRX 0x07 01 # response\n\n01 02\n").unwrap();

        assert_eq!(capture.len(), 3);
        assert_eq!(capture[0].direction, Some(Direction::Tx));
        assert_eq!(capture[0].bytes, [0x07, 0x0A, 0xFF, 0x00]);
        assert_eq!(capture[1].direction, Some(Direction::Rx));
        assert_eq!(capture[2].direction, None);
    }

    #[test]
    fn reports_the_line_and_invalid_token() {
        assert_eq!(
            parse_hex_capture("# header\nTX 07 XYZ"),
            Err(CaptureParseError::InvalidByte {
                line: 2,
                token: "XYZ".to_owned(),
            })
        );
    }

    #[test]
    fn diffs_offsets_lengths_and_missing_reports() {
        let before = parse_hex_capture("TX 07 0A 00\nRX 07 01").unwrap();
        let after = parse_hex_capture("TX 07 0A FF 10\n").unwrap();
        let diff = diff_captures(&before, &after);

        assert_eq!(diff.len(), 2);
        assert_eq!(
            diff[0].changes,
            [
                ByteChange {
                    offset: 2,
                    before: Some(0),
                    after: Some(0xFF),
                },
                ByteChange {
                    offset: 3,
                    before: None,
                    after: Some(0x10),
                },
            ]
        );
        assert_eq!(diff[1].before_direction, Some(Direction::Rx));
        assert_eq!(diff[1].after_len, 0);
    }
}

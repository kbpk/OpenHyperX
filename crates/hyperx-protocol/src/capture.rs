use thiserror::Error;

use crate::Direction;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedReport {
    pub direction: Option<Direction>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureTextFormat {
    HexReports,
    OpenHyperXTrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureRecord {
    pub line: usize,
    pub interface: Option<i32>,
    pub report: CapturedReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedReportLog {
    pub format: CaptureTextFormat,
    pub records: Vec<CaptureRecord>,
    pub ignored_lines: usize,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ReportLogParseError {
    #[error("line {line}: {message}")]
    Invalid { line: usize, message: String },
    #[error(
        "no raw HID reports found; use UTF-8 hex lines or an OpenHyperX --trace log (not pcapng)"
    )]
    NoReports,
}

/// Parse UTF-8 hex exports or our textual tracing format without performing I/O.
/// Trace mode ignores console/discovery lines but NEVER ignores a malformed
/// `raw HID report` line. Source line/order/interface are retained. No timestamp
/// is inferred, no reports are joined, and no replay path is provided.
pub fn parse_report_log(input: &str) -> Result<ParsedReportLog, ReportLogParseError> {
    let is_trace = input.lines().any(|line| line.contains("raw HID report"));
    let mut records = Vec::new();
    let mut ignored_lines = 0;
    for (index, original) in input.lines().enumerate() {
        let line_number = index + 1;
        let clean = strip_ansi(original);
        let line = clean.trim().trim_start_matches('\u{feff}').trim();
        let invalid = |message: String| ReportLogParseError::Invalid {
            line: line_number,
            message,
        };
        let (interface, report) = if is_trace {
            let Some((_, fields)) = line.split_once("raw HID report") else {
                ignored_lines += 1;
                continue;
            };
            let (metadata, bytes) = fields
                .split_once("bytes=")
                .ok_or_else(|| invalid("missing bytes field".into()))?;
            let field = |name: &str| -> Result<&str, ReportLogParseError> {
                let prefix = format!("{name}=");
                let mut values = metadata
                    .split_whitespace()
                    .filter_map(|token| token.strip_prefix(&prefix));
                let value = values
                    .next()
                    .ok_or_else(|| invalid(format!("missing {name} field")))?;
                if values.next().is_some() {
                    return Err(invalid(format!("duplicate {name} field")));
                }
                Ok(value.trim_matches('"'))
            };
            let direction = match field("direction")? {
                "TX" => Direction::Tx,
                "RX" => Direction::Rx,
                other => return Err(invalid(format!("invalid direction {other:?}"))),
            };
            let interface = field("interface")?
                .parse::<i32>()
                .map_err(|_| invalid("invalid interface number".into()))?;
            let id = field("report_id")?;
            let id = id
                .strip_prefix("0x")
                .or_else(|| id.strip_prefix("0X"))
                .unwrap_or(id);
            let report_id =
                u8::from_str_radix(id, 16).map_err(|_| invalid("invalid report_id".into()))?;
            let mut parsed =
                parse_hex_capture(bytes).map_err(|error| invalid(error.to_string()))?;
            let mut report = parsed
                .pop()
                .ok_or_else(|| invalid("empty bytes field".into()))?;
            if report.bytes.first() != Some(&report_id) {
                return Err(invalid("report_id differs from first payload byte".into()));
            }
            if report.direction.is_some() {
                return Err(invalid(
                    "bytes field must contain only hexadecimal bytes".into(),
                ));
            }
            report.direction = Some(direction);
            (Some(interface), report)
        } else {
            let mut parsed = parse_hex_capture(line).map_err(|error| invalid(error.to_string()))?;
            let Some(report) = parsed.pop() else {
                ignored_lines += 1;
                continue;
            };
            (None, report)
        };
        records.push(CaptureRecord {
            line: line_number,
            interface,
            report,
        });
    }
    if records.is_empty() {
        return Err(ReportLogParseError::NoReports);
    }
    Ok(ParsedReportLog {
        format: if is_trace {
            CaptureTextFormat::OpenHyperXTrace
        } else {
            CaptureTextFormat::HexReports
        },
        records,
        ignored_lines,
    })
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' && characters.peek() == Some(&'[') {
            characters.next();
            for next in characters.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            output.push(character);
        }
    }
    output
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

    #[test]
    fn parses_trace_with_ansi_bom_crlf_and_keeps_interface_direction_and_line() {
        let input = "\u{feff}TRACE enumerated collection\r\n\u{1b}[35mTRACE\u{1b}[0m raw HID report direction=\"TX\" interface=1 report_id=0x07 bytes=07 81 00\r\nError: stopped\r\nTRACE raw HID report direction=\"RX\" interface=2 report_id=0x00 bytes=00 00 07 81\r\n";
        let log = parse_report_log(input).unwrap();
        assert_eq!(log.format, CaptureTextFormat::OpenHyperXTrace);
        assert_eq!(log.ignored_lines, 2);
        assert_eq!(log.records[0].line, 2);
        assert_eq!(log.records[0].interface, Some(1));
        assert_eq!(log.records[0].report.direction, Some(Direction::Tx));
        assert_eq!(log.records[1].interface, Some(2));
        assert_eq!(log.records[1].report.bytes, [0, 0, 7, 0x81]);
    }

    #[test]
    fn parses_hex_exports_and_never_invents_interface_or_direction() {
        let log = parse_report_log("\u{feff}# capture\nTX 07 81\n07 01 04\n").unwrap();
        assert_eq!(log.format, CaptureTextFormat::HexReports);
        assert_eq!(log.records[0].line, 2);
        assert_eq!(log.records[0].interface, None);
        assert_eq!(log.records[1].report.direction, None);
        assert_eq!(
            parse_report_log("# no packets\n"),
            Err(ReportLogParseError::NoReports)
        );
    }

    #[test]
    fn malformed_trace_packets_fail_loudly_instead_of_disappearing() {
        for fields in [
            "direction=TX interface=1 report_id=0x07 bytes=07 ZZ",
            "direction=TX interface=1 report_id=0x08 bytes=07 81",
            "direction=TX interface=1 bytes=07 81",
            "direction=TX direction=RX interface=1 report_id=0x07 bytes=07 81",
            "direction=WRONG interface=1 report_id=0x07 bytes=07 81",
            "direction=TX interface=no report_id=0x07 bytes=07 81",
            "direction=TX interface=1 report_id=0x07",
            "direction=TX interface=1 report_id=0x07 bytes=TX 07 81",
        ] {
            assert!(
                matches!(
                    parse_report_log(&format!("console\nTRACE raw HID report {fields}")),
                    Err(ReportLogParseError::Invalid { line: 2, .. })
                ),
                "{fields}"
            );
        }
        assert!(parse_report_log("TX 07 XYZ").is_err());
    }
}

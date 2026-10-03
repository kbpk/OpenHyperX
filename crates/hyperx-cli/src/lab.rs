//! Narrow lab diagnostics, independent of the suspended runtime openers.
//! No raw-send facility, selectable report ID, arbitrary selector, startup or settings write.
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use hyperx_core::HidInterfaceInfo;
use hyperx_devices::PULSEFIRE_RAID;
use hyperx_hid::{HidApiTransport, HidDiscovery, HidTransport};
use hyperx_protocol::pulsefire_raid::{
    encode_profile_read_request, encode_runtime_profile_read_prelude, PerformanceProfile,
    ProfileSection, DIRECT_REPORT_ID, DIRECT_REPORT_LENGTH,
};

const READ_REQUEST_DELAY: Duration = Duration::from_millis(110);

#[derive(Debug, Subcommand)]
pub enum LabCommand {
    /// Read one feature response, without any preceding SET_REPORT. Windows lab only.
    #[command(name = "raid-feature-get")]
    FeatureGet {
        /// Explicit consent to the isolated diagnostic; not an override for runtime access.
        #[arg(long = "unsafe", required = true)]
        unsafe_confirmation: bool,
    },
    /// Send only the captured 07 81 read-request, then GET; no profile selector. Windows lab only.
    #[command(name = "raid-read-request-get")]
    ReadRequestGet {
        /// Consent to one request/GET probe; never an override for runtime access.
        #[arg(long = "unsafe", required = true)]
        unsafe_confirmation: bool,
    },
    /// Send only captured 07 03 04 64; MAY disable cursor/clicks/lighting. No request or GET.
    #[command(name = "raid-runtime-select-only")]
    RuntimeSelectOnly {
        /// Consent to the isolated potentially disruptive selector; not runtime access.
        #[arg(long = "unsafe", required = true)]
        unsafe_confirmation: bool,
    },
}

#[derive(Clone, Copy)]
enum Probe {
    PassiveGet,
    ReadRequestGet,
    RuntimeSelectOnly,
}

#[derive(Debug)]
enum FeatureObservation {
    Onboard(PerformanceProfile),
    Runtime(PerformanceProfile),
    Opaque(String),
}

fn classify_feature_response(report: &[u8; DIRECT_REPORT_LENGTH]) -> FeatureObservation {
    let profile = match PerformanceProfile::parse(report) {
        Ok(profile) => profile,
        Err(error) => return FeatureObservation::Opaque(error.to_string()),
    };
    if let Err(error) = profile.validate_confirmed_read_settings() {
        return FeatureObservation::Opaque(error.to_string());
    }
    match profile.section() {
        ProfileSection::Onboard => FeatureObservation::Onboard(profile),
        ProfileSection::Runtime => FeatureObservation::Runtime(profile),
    }
}

pub fn run(command: LabCommand, discovery: &dyn HidDiscovery) -> Result<()> {
    let (unsafe_confirmation, probe) = match command {
        LabCommand::FeatureGet {
            unsafe_confirmation,
        } => (unsafe_confirmation, Probe::PassiveGet),
        LabCommand::ReadRequestGet {
            unsafe_confirmation,
        } => (unsafe_confirmation, Probe::ReadRequestGet),
        LabCommand::RuntimeSelectOnly {
            unsafe_confirmation,
        } => (unsafe_confirmation, Probe::RuntimeSelectOnly),
    };
    if !unsafe_confirmation {
        bail!("the lab diagnostic requires explicit --unsafe; no HID access attempted");
    }
    if !cfg!(windows) {
        bail!("this isolated lab diagnostic is Windows-only; no HID access attempted");
    }
    let operation = match probe {
        Probe::PassiveGet => "Exactly one GET_REPORT (ID 0x07, 264 bytes); no SET_REPORT or selector",
        Probe::ReadRequestGet => "Exactly one SET_REPORT 07 81 + zero-fill (264 bytes), wait 110 ms, then one GET_REPORT (ID 0x07, 264 bytes); no profile selector. The standalone request's physical effect is under investigation",
        Probe::RuntimeSelectOnly => "Exactly one SET_REPORT 07 03 04 64 + zero-fill (264 bytes). MAY disable cursor, clicks and lighting by activating runtime; have physical USB reconnect ready. No 81 request, GET_REPORT or further vendor report",
    };
    eprintln!("WARNING: isolated lab diagnostic. Close NGENUITY/OpenRGB first and have USB reconnect available. {operation}; no initializer, profile/RGB image write, ACK probe, retry or onboard save. Normal runtime access remains blocked.");
    let interfaces = discovery.enumerate()?;
    let configuration = super::select_configuration_interface(&interfaces)?;
    validate_target(configuration)?;
    let mut transport = HidApiTransport::open(configuration)
        .context("failed to open the confirmed Raid configuration collection for the lab probe")?;
    // A descriptor check is a standard metadata operation, never a vendor
    // SET_REPORT. Refuse changed framing rather than probing a sibling device.
    let descriptor = transport.report_descriptor()?;
    validate_descriptor(&descriptor)?;
    let report = match probe {
        Probe::PassiveGet => read_one_feature_response(&mut transport)?,
        Probe::ReadRequestGet => read_request_then_get(&mut transport, std::thread::sleep)?,
        Probe::RuntimeSelectOnly => {
            select_runtime_once(&mut transport)?;
            println!("Lab selector SET_REPORT completed: 264 bytes, prefix 07 03 04 64. No GET_REPORT, 81 request or other vendor report was sent.");
            println!("Transport completion is not a profile read, ACK, safety or persistence check. STOP and check physical behavior; no retry, software restoration or initialization. Normal runtime access remains blocked.");
            return Ok(());
        }
    };
    println!(
        "Lab GET_REPORT completed: {} bytes, prefix {}.",
        report.len(),
        hyperx_protocol::format_hex(&report[..3])
    );
    println!(
        "Nonzero bytes after offset 2: {}.",
        report[3..].iter().filter(|&&byte| byte != 0).count()
    );
    match classify_feature_response(&report) {
        FeatureObservation::Onboard(profile) => {
            println!("Observed onboard-section image: all currently decoded setting fields are valid. This is not a verified fresh onboard snapshot or current runtime state.");
            super::print_profile_settings(&profile);
        }
        FeatureObservation::Runtime(profile) => {
            println!("Observed runtime-section image: all currently decoded setting fields are valid. This is not a verified fresh/current runtime snapshot or a safe write baseline.");
            super::print_profile_settings(&profile);
        }
        FeatureObservation::Opaque(reason) => {
            println!("Opaque or unusable feature response ({reason}); no settings inferred.");
        }
    }
    println!("The response section reflects the device's session state; no selector was sent. Known-field validation does not interpret opaque bytes, prove freshness or permit a write. Inspect the capture and physical behavior before any further experiment. No automatic retry or restoration.");
    println!(
        "Feature SET_REPORT count: {} (only the fixed 07 81 request when enabled).",
        usize::from(matches!(probe, Probe::ReadRequestGet))
    );
    Ok(())
}

fn select_runtime_once(transport: &mut dyn HidTransport) -> Result<()> {
    if transport.interface_number() != 1 {
        bail!("lab selector requires configuration interface 1; no vendor report requested");
    }
    // The complete 07 03 04 64 packet matches two retained Legacy launches.
    // Its standalone physical effect is the experiment, NOT a claim that it
    // harmlessly selects a read address. Do not attach GET/81 or any recovery.
    let report = encode_runtime_profile_read_prelude();
    let size = transport
        .send_feature_report(&report)
        .context("single lab selector failed; STOP without retry or any further report")?;
    if size != report.len() {
        bail!(
            "single lab selector short write ({size}/{}); STOP without retry or any further report",
            report.len()
        );
    }
    Ok(())
}

fn read_request_then_get(
    transport: &mut dyn HidTransport,
    mut wait: impl FnMut(Duration),
) -> Result<[u8; DIRECT_REPORT_LENGTH]> {
    if transport.interface_number() != 1 {
        bail!("lab request/GET requires configuration interface 1; no vendor report requested");
    }
    // Exact full request matched in two Legacy launch captures; see research.md.
    // Intentionally omit 07 03 selection and session startup. The current
    // section is unknown: never infer a runtime baseline from this response.
    let request = encode_profile_read_request();
    let size = transport
        .send_feature_report(&request)
        .context("single lab read-request failed; STOP without retry or GET")?;
    if size != request.len() {
        bail!(
            "single lab read-request short write ({size}/{}); STOP without retry or GET",
            request.len()
        );
    }
    wait(READ_REQUEST_DELAY);
    read_one_feature_response(transport)
}

fn validate_target(info: &HidInterfaceInfo) -> Result<()> {
    if info.usb_id() != PULSEFIRE_RAID.usb_id
        || !info.matches(PULSEFIRE_RAID.configuration_interface)
        || info.release_number != 0x1124
    {
        bail!("lab GET is restricted to Raid 0951:16E4 release 1124, interface 1, usage FF01:0001; no vendor report requested");
    }
    Ok(())
}

fn validate_descriptor(descriptor: &[u8]) -> Result<()> {
    // Exact locally read descriptor in docs/research.md (24 bytes). The 263-byte
    // payload plus ID is the same GET framing captured before the incident.
    const CONFIRMED: &[u8] = &[
        0x06, 0x01, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x85, 0x07, 0x09, 0x20, 0x15, 0x00, 0x26, 0xFF,
        0x00, 0x75, 0x08, 0x96, 0x07, 0x01, 0xB1, 0x02, 0xC0,
    ];
    if descriptor != CONFIRMED {
        bail!("lab GET descriptor differs from confirmed release-1124 framing; no vendor report requested");
    }
    Ok(())
}

fn read_one_feature_response(
    transport: &mut dyn HidTransport,
) -> Result<[u8; DIRECT_REPORT_LENGTH]> {
    if transport.interface_number() != 1 {
        bail!("lab GET requires configuration interface 1; no vendor report requested");
    }
    let mut report = [0; DIRECT_REPORT_LENGTH];
    report[0] = DIRECT_REPORT_ID;
    // Deliberately no prelude/request/initializer and no attempt to decode this
    // as a snapshot. A passive response could be stale, zero-filled or opaque.
    let size = transport
        .get_feature_report(&mut report)
        .context("single lab GET_REPORT failed; STOP without retry")?;
    if size != DIRECT_REPORT_LENGTH || report[0] != DIRECT_REPORT_ID {
        bail!("single lab GET_REPORT returned unexpected framing (length {size}, ID 0x{:02X}); STOP without retry", report[0]);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperx_hid::{testing::MockHidTransport, HidError};
    use hyperx_protocol::capture::parse_report_log;

    fn fixture_response(source: &str, index: usize) -> [u8; DIRECT_REPORT_LENGTH] {
        let log = parse_report_log(source).unwrap();
        log.records[index]
            .report
            .bytes
            .as_slice()
            .try_into()
            .unwrap()
    }

    #[test]
    fn captured_selector_free_request_is_observed_onboard_not_runtime() {
        let response = fixture_response(
            include_str!("../../hyperx-protocol/tests/fixtures/read-request-get-onboard.hex"),
            1,
        );
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        let mut waits = Vec::new();
        let received = read_request_then_get(&mut transport, |delay| waits.push(delay)).unwrap();
        assert_eq!(received, response);
        assert!(matches!(
            classify_feature_response(&received),
            FeatureObservation::Onboard(_)
        ));
        assert_eq!(waits, [READ_REQUEST_DELAY]);
        assert_eq!(transport.feature_reports_sent().len(), 1);
        assert_eq!(transport.feature_read_attempts(), 1);
        transport.assert_drained();
    }

    #[test]
    fn classification_rejects_passive_opaque_empty_invalid_and_host_write_images() {
        let mut opaque = [0; DIRECT_REPORT_LENGTH];
        opaque[..4].copy_from_slice(&[7, 0xFF, 0xFF, 0xFF]);
        assert!(matches!(
            classify_feature_response(&opaque),
            FeatureObservation::Opaque(_)
        ));
        let empty = fixture_response(
            include_str!("../../hyperx-protocol/tests/fixtures/cold-legacy-startup-images.hex"),
            0,
        );
        assert!(matches!(
            classify_feature_response(&empty),
            FeatureObservation::Opaque(_)
        ));
        let onboard = fixture_response(
            include_str!("../../hyperx-protocol/tests/fixtures/read-request-get-onboard.hex"),
            1,
        );
        let mut invalid_polling = onboard;
        invalid_polling[0x18] = 3;
        assert!(matches!(
            classify_feature_response(&invalid_polling),
            FeatureObservation::Opaque(_)
        ));
        let mut host_write = onboard;
        host_write[1] = 1;
        assert!(matches!(
            classify_feature_response(&host_write),
            FeatureObservation::Opaque(_)
        ));
    }

    #[test]
    fn captured_warm_runtime_response_is_observation_not_write_authority() {
        let response = fixture_response(
            include_str!("../../hyperx-protocol/tests/fixtures/warm-legacy-startup-images.hex"),
            0,
        );
        assert!(matches!(
            classify_feature_response(&response),
            FeatureObservation::Runtime(_)
        ));
    }

    #[test]
    fn passive_get_reads_once_without_any_tx_even_for_empty_or_opaque_response() {
        for header in [[7, 0x81, 4], [7, 0x81, 1], [7, 0x99, 0xFE]] {
            let mut response = [0; DIRECT_REPORT_LENGTH];
            response[..3].copy_from_slice(&header);
            let mut transport = MockHidTransport::new(1);
            transport.queue_feature_response(response);
            assert_eq!(read_one_feature_response(&mut transport).unwrap(), response);
            assert!(transport.feature_reports_sent().is_empty());
            assert_eq!(transport.feature_read_attempts(), 1);
            transport.assert_drained();
        }
    }

    #[test]
    fn passive_get_error_or_short_response_never_retries_or_initializes() {
        for short in [true, false] {
            let mut transport = MockHidTransport::new(1);
            if short {
                transport.queue_feature_response([7, 0x81, 4]);
            } else {
                transport.queue_feature_error(HidError::Transport("injected GET failure".into()));
            }
            assert!(read_one_feature_response(&mut transport).is_err());
            assert!(transport.feature_reports_sent().is_empty());
            assert_eq!(transport.feature_read_attempts(), 1);
            transport.assert_drained();
        }
        let mut wrong_interface = MockHidTransport::new(0);
        assert!(read_one_feature_response(&mut wrong_interface).is_err());
        assert_eq!(wrong_interface.feature_read_attempts(), 0);
        assert!(wrong_interface.feature_reports_sent().is_empty());
    }

    #[test]
    fn missing_confirmation_fails_before_discovery() {
        struct NeverDiscover;
        impl HidDiscovery for NeverDiscover {
            fn enumerate(&self) -> Result<Vec<HidInterfaceInfo>, HidError> {
                panic!("unconfirmed probe attempted discovery");
            }
        }
        for command in [
            LabCommand::FeatureGet {
                unsafe_confirmation: false,
            },
            LabCommand::ReadRequestGet {
                unsafe_confirmation: false,
            },
            LabCommand::RuntimeSelectOnly {
                unsafe_confirmation: false,
            },
        ] {
            assert!(run(command, &NeverDiscover).is_err());
        }
    }

    #[test]
    fn isolated_selector_sends_only_the_exact_golden_without_reads_or_recovery() {
        let mut golden = [0; 264];
        golden[..4].copy_from_slice(&[7, 3, 4, 0x64]);
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(golden);
        select_runtime_once(&mut transport).unwrap();
        assert_eq!(transport.feature_reports_sent(), &[golden.to_vec()]);
        assert_eq!(transport.feature_read_attempts(), 0);
        transport.assert_drained();
    }

    #[test]
    fn isolated_selector_failure_never_adds_request_get_initializer_or_recovery() {
        for result in [
            Ok(263),
            Err(HidError::Transport("injected selector failure".into())),
        ] {
            let mut transport = MockHidTransport::new(1);
            transport.expect_feature_report_result(encode_runtime_profile_read_prelude(), result);
            assert!(select_runtime_once(&mut transport).is_err());
            assert_eq!(transport.feature_reports_sent().len(), 1);
            assert_eq!(transport.feature_read_attempts(), 0);
            transport.assert_drained();
        }
        let mut wrong = MockHidTransport::new(0);
        assert!(select_runtime_once(&mut wrong).is_err());
        assert!(wrong.feature_reports_sent().is_empty());
        assert_eq!(wrong.feature_read_attempts(), 0);
    }

    #[test]
    fn request_get_sends_exactly_the_captured_request_and_preserves_opaque_rx() {
        let mut golden = [0; 264];
        golden[..2].copy_from_slice(&[7, 0x81]);
        for header in [[7, 0x81, 1], [7, 0x81, 4], [7, 255, 255]] {
            let mut response = [0; 264];
            response[..3].copy_from_slice(&header);
            let mut transport = MockHidTransport::new(1);
            transport.expect_feature_report(golden);
            transport.queue_feature_response(response);
            let mut waits = Vec::new();
            assert_eq!(
                read_request_then_get(&mut transport, |delay| waits.push(delay)).unwrap(),
                response
            );
            assert_eq!(waits, [Duration::from_millis(110)]);
            assert_eq!(transport.feature_reports_sent(), &[golden.to_vec()]);
            assert_eq!(transport.feature_read_attempts(), 1);
            transport.assert_drained();
        }
    }

    #[test]
    fn request_tx_failure_stops_before_wait_get_and_any_retry() {
        for result in [
            Ok(263),
            Err(HidError::Transport("injected TX error".into())),
        ] {
            let mut transport = MockHidTransport::new(1);
            transport.expect_feature_report_result(encode_profile_read_request(), result);
            assert!(
                read_request_then_get(&mut transport, |_| panic!("wait after failed TX")).is_err()
            );
            assert_eq!(transport.feature_reports_sent().len(), 1);
            assert_eq!(transport.feature_read_attempts(), 0);
            transport.assert_drained();
        }
        let mut wrong = MockHidTransport::new(0);
        assert!(read_request_then_get(&mut wrong, |_| panic!("wait on wrong interface")).is_err());
        assert!(wrong.feature_reports_sent().is_empty());
        assert_eq!(wrong.feature_read_attempts(), 0);
    }

    #[test]
    fn request_rx_failure_does_not_select_initialize_retry_or_restore() {
        for short in [true, false] {
            let mut transport = MockHidTransport::new(1);
            transport.expect_feature_report(encode_profile_read_request());
            if short {
                transport.queue_feature_response([7, 0x81, 1]);
            } else {
                transport.queue_feature_error(HidError::Transport("injected GET error".into()));
            }
            let mut waits = Vec::new();
            assert!(read_request_then_get(&mut transport, |delay| waits.push(delay)).is_err());
            assert_eq!(waits, [READ_REQUEST_DELAY]);
            assert_eq!(transport.feature_reports_sent().len(), 1);
            assert_eq!(transport.feature_read_attempts(), 1);
            transport.assert_drained();
        }
    }

    #[test]
    fn lab_rejects_other_models_releases_collections_and_descriptors() {
        let mut info = super::super::tests::fixture();
        info.interface_number = 1;
        info.usage_page = 0xFF01;
        info.usage = 1;
        info.release_number = 0x1124;
        validate_target(&info).unwrap();
        for change in 0..5 {
            let mut wrong = info.clone();
            match change {
                0 => wrong.vendor_id = 0x03F0,
                1 => wrong.product_id = 0xFFFF,
                2 => wrong.release_number = 0x1125,
                3 => wrong.interface_number = 0,
                _ => wrong.usage_page = 1,
            }
            assert!(validate_target(&wrong).is_err());
        }
        assert!(validate_descriptor(&[]).is_err());
        assert!(validate_descriptor(&[0x85, 0x07]).is_err());
    }
}

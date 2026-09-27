//! HID transport boundary.
//!
//! Milestone 1 deliberately exposes enumeration only. Opening devices and
//! sending feature reports will be added with protocol-backed safety checks.

use std::ffi::CString;

use hyperx_core::HidInterfaceInfo;
use hyperx_protocol::{trace_report, Direction};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HidError {
    #[error("HID backend error: {0}")]
    Backend(#[from] hidapi::HidError),

    #[error("HID path contains an embedded NUL byte")]
    InvalidPath,

    #[error("HID transport error: {0}")]
    Transport(String),
}

pub trait HidDiscovery {
    fn enumerate(&self) -> Result<Vec<HidInterfaceInfo>, HidError>;
}

/// Protocol-agnostic report transport. Device drivers depend on this trait,
/// rather than directly on hidapi, so packet tests need no physical hardware.
pub trait HidTransport {
    fn interface_number(&self) -> i32;
    fn send_feature_report(&mut self, report: &[u8]) -> Result<usize, HidError>;
    fn get_feature_report(&mut self, report: &mut [u8]) -> Result<usize, HidError>;
    fn write(&mut self, report: &[u8]) -> Result<usize, HidError>;
    fn read_timeout(&mut self, report: &mut [u8], timeout_ms: i32) -> Result<usize, HidError>;
    fn report_descriptor(&self) -> Result<Vec<u8>, HidError>;
}

#[derive(Default)]
pub struct HidApiDiscovery;

pub struct HidApiTransport {
    device: hidapi::HidDevice,
    interface_number: i32,
}

impl HidApiTransport {
    pub fn open(info: &HidInterfaceInfo) -> Result<Self, HidError> {
        let path = CString::new(info.path.as_bytes()).map_err(|_| HidError::InvalidPath)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        Ok(Self {
            device,
            interface_number: info.interface_number,
        })
    }
}

impl HidTransport for HidApiTransport {
    fn interface_number(&self) -> i32 {
        self.interface_number
    }

    fn send_feature_report(&mut self, report: &[u8]) -> Result<usize, HidError> {
        trace_report(Direction::Tx, self.interface_number, report);
        self.device.send_feature_report(report)?;
        Ok(report.len())
    }

    fn get_feature_report(&mut self, report: &mut [u8]) -> Result<usize, HidError> {
        let size = self.device.get_feature_report(report)?;
        trace_report(Direction::Rx, self.interface_number, &report[..size]);
        Ok(size)
    }

    fn write(&mut self, report: &[u8]) -> Result<usize, HidError> {
        trace_report(Direction::Tx, self.interface_number, report);
        Ok(self.device.write(report)?)
    }

    fn read_timeout(&mut self, report: &mut [u8], timeout_ms: i32) -> Result<usize, HidError> {
        let size = self.device.read_timeout(report, timeout_ms)?;
        if size != 0 {
            trace_report(Direction::Rx, self.interface_number, &report[..size]);
        }
        Ok(size)
    }

    fn report_descriptor(&self) -> Result<Vec<u8>, HidError> {
        let mut descriptor = vec![0_u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
        let size = self.device.get_report_descriptor(&mut descriptor)?;
        descriptor.truncate(size);
        Ok(descriptor)
    }
}

impl HidDiscovery for HidApiDiscovery {
    fn enumerate(&self) -> Result<Vec<HidInterfaceInfo>, HidError> {
        let api = hidapi::HidApi::new()?;
        let mut interfaces = Vec::new();

        for device in api.device_list() {
            let info = HidInterfaceInfo {
                path: device.path().to_string_lossy().into_owned(),
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                release_number: device.release_number(),
                interface_number: device.interface_number(),
                usage_page: device.usage_page(),
                usage: device.usage(),
                manufacturer: non_empty(device.manufacturer_string()),
                product: non_empty(device.product_string()),
                serial_number: non_empty(device.serial_number()),
            };

            tracing::trace!(
                target: "hyperx_hid::discovery",
                vid = format_args!("0x{:04X}", info.vendor_id),
                pid = format_args!("0x{:04X}", info.product_id),
                interface = info.interface_number,
                usage_page = format_args!("0x{:04X}", info.usage_page),
                usage = format_args!("0x{:04X}", info.usage),
                path = %info.path,
                "enumerated HID collection"
            );

            interfaces.push(info);
        }

        Ok(interfaces)
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

pub mod testing {
    use std::collections::VecDeque;

    use super::*;

    pub struct MockHidDiscovery {
        interfaces: Vec<HidInterfaceInfo>,
    }

    impl MockHidDiscovery {
        pub fn new(interfaces: Vec<HidInterfaceInfo>) -> Self {
            Self { interfaces }
        }
    }

    impl HidDiscovery for MockHidDiscovery {
        fn enumerate(&self) -> Result<Vec<HidInterfaceInfo>, HidError> {
            Ok(self.interfaces.clone())
        }
    }

    /// Scriptable transport for unit and golden-packet tests.
    #[derive(Debug, Default)]
    pub struct MockHidTransport {
        interface_number: i32,
        expected_feature_tx: VecDeque<(Vec<u8>, Result<usize, HidError>)>,
        feature_rx: VecDeque<Result<Vec<u8>, HidError>>,
        feature_tx_attempts: Vec<Vec<u8>>,
        feature_read_attempts: usize,
        descriptor: Option<Vec<u8>>,
        expected_output_tx: VecDeque<Vec<u8>>,
        input_rx: VecDeque<Vec<u8>>,
    }

    impl MockHidTransport {
        pub fn new(interface_number: i32) -> Self {
            Self {
                interface_number,
                ..Self::default()
            }
        }

        pub fn expect_feature_report(&mut self, report: impl Into<Vec<u8>>) {
            let report = report.into();
            let length = report.len();
            self.expect_feature_report_result(report, Ok(length));
        }

        pub fn set_report_descriptor(&mut self, descriptor: impl Into<Vec<u8>>) {
            self.descriptor = Some(descriptor.into());
        }

        /// Match the exact packet but inject a short write or transport failure.
        pub fn expect_feature_report_result(
            &mut self,
            report: impl Into<Vec<u8>>,
            result: Result<usize, HidError>,
        ) {
            self.expected_feature_tx.push_back((report.into(), result));
        }

        pub fn queue_feature_response(&mut self, report: impl Into<Vec<u8>>) {
            self.feature_rx.push_back(Ok(report.into()));
        }

        pub fn queue_feature_error(&mut self, error: HidError) {
            self.feature_rx.push_back(Err(error));
        }

        /// Includes failed/unexpected attempts: an empty script alone cannot
        /// prove the driver stopped sending after a failure.
        pub fn feature_reports_sent(&self) -> &[Vec<u8>] {
            &self.feature_tx_attempts
        }

        pub const fn feature_read_attempts(&self) -> usize {
            self.feature_read_attempts
        }

        pub fn expect_output_report(&mut self, report: impl Into<Vec<u8>>) {
            self.expected_output_tx.push_back(report.into());
        }

        pub fn queue_input_report(&mut self, report: impl Into<Vec<u8>>) {
            self.input_rx.push_back(report.into());
        }

        pub fn assert_drained(&self) {
            assert!(
                self.expected_feature_tx.is_empty()
                    && self.feature_rx.is_empty()
                    && self.expected_output_tx.is_empty()
                    && self.input_rx.is_empty(),
                "mock HID script was not fully consumed: {self:?}"
            );
        }

        fn check_tx(queue: &mut VecDeque<Vec<u8>>, actual: &[u8]) -> Result<usize, HidError> {
            let expected = queue.pop_front().ok_or_else(|| {
                HidError::Transport("unexpected report: no transmission was queued".to_owned())
            })?;
            if expected != actual {
                return Err(HidError::Transport(format!(
                    "report mismatch: expected {expected:02X?}, got {actual:02X?}"
                )));
            }
            Ok(actual.len())
        }

        fn receive(queue: &mut VecDeque<Vec<u8>>, output: &mut [u8]) -> Result<usize, HidError> {
            let response = queue
                .pop_front()
                .ok_or_else(|| HidError::Transport("no mock response is queued".to_owned()))?;
            if response.len() > output.len() {
                return Err(HidError::Transport(format!(
                    "mock response has {} bytes but buffer has {}",
                    response.len(),
                    output.len()
                )));
            }
            output[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    impl HidTransport for MockHidTransport {
        fn interface_number(&self) -> i32 {
            self.interface_number
        }

        fn send_feature_report(&mut self, report: &[u8]) -> Result<usize, HidError> {
            self.feature_tx_attempts.push(report.to_vec());
            let (expected, result) = self.expected_feature_tx.pop_front().ok_or_else(|| {
                HidError::Transport("unexpected report: no transmission was queued".to_owned())
            })?;
            if expected != report {
                return Err(HidError::Transport(format!(
                    "report mismatch: expected {expected:02X?}, got {report:02X?}"
                )));
            }
            result
        }

        fn get_feature_report(&mut self, report: &mut [u8]) -> Result<usize, HidError> {
            self.feature_read_attempts += 1;
            if self.feature_rx.front().is_some_and(Result::is_err) {
                return Err(self.feature_rx.pop_front().unwrap().unwrap_err());
            }
            let expected_report_id = self
                .feature_rx
                .front()
                .and_then(|response| response.as_ref().ok())
                .and_then(|response| response.first())
                .copied()
                .ok_or_else(|| {
                    HidError::Transport("no non-empty mock feature response is queued".to_owned())
                })?;
            if report.first().copied() != Some(expected_report_id) {
                return Err(HidError::Transport(format!(
                    "feature GET_REPORT ID mismatch: expected 0x{expected_report_id:02X}, got {:?}",
                    report.first()
                )));
            }
            let response = self.feature_rx.pop_front().unwrap()?;
            if response.len() > report.len() {
                return Err(HidError::Transport(format!(
                    "mock response has {} bytes but buffer has {}",
                    response.len(),
                    report.len()
                )));
            }
            report[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }

        fn write(&mut self, report: &[u8]) -> Result<usize, HidError> {
            Self::check_tx(&mut self.expected_output_tx, report)
        }

        fn read_timeout(&mut self, report: &mut [u8], _timeout_ms: i32) -> Result<usize, HidError> {
            Self::receive(&mut self.input_rx, report)
        }

        fn report_descriptor(&self) -> Result<Vec<u8>, HidError> {
            self.descriptor.clone().ok_or_else(|| {
                HidError::Transport("no mock report descriptor is configured".to_owned())
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::MockHidTransport, HidTransport};

    #[test]
    fn mock_transport_verifies_tx_and_supplies_rx() {
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report([0x07, 0x0A, 0xA0]);
        transport.queue_feature_response([0x07, 0x01]);

        assert_eq!(
            transport.send_feature_report(&[0x07, 0x0A, 0xA0]).unwrap(),
            3
        );
        let mut response = [0_u8; 8];
        response[0] = 0x07;
        assert_eq!(transport.get_feature_report(&mut response).unwrap(), 2);
        assert_eq!(&response[..2], &[0x07, 0x01]);
        transport.assert_drained();
    }

    #[test]
    fn mock_transport_rejects_an_unexpected_packet() {
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report([0x07, 0x0A]);

        let error = transport.send_feature_report(&[0x07, 0x0B]).unwrap_err();
        assert!(error.to_string().contains("report mismatch"));
        // A consumed script must not erase evidence of the failed attempt.
        assert_eq!(transport.feature_reports_sent(), &[vec![0x07, 0x0B]]);
    }

    #[test]
    fn mock_transport_verifies_feature_report_id_on_get() {
        let mut transport = MockHidTransport::new(1);
        transport.queue_feature_response([0x07, 0x81]);
        let mut response = [0_u8; 8];
        response[0] = 0x06;

        let error = transport.get_feature_report(&mut response).unwrap_err();
        assert!(error.to_string().contains("GET_REPORT ID mismatch"));
    }

    #[test]
    fn mock_input_reports_do_not_require_a_prefilled_report_id() {
        let mut transport = MockHidTransport::new(1);
        transport.queue_input_report([0x03, 0xAA]);
        let mut response = [0_u8; 8];

        assert_eq!(transport.read_timeout(&mut response, 50).unwrap(), 2);
        assert_eq!(&response[..2], &[0x03, 0xAA]);
        transport.assert_drained();
    }

    #[test]
    fn mock_feature_reports_inject_short_writes_and_errors_without_losing_history() {
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report_result([7, 1, 4], Ok(2));
        transport.expect_feature_report_result(
            [7, 5, 4],
            Err(super::HidError::Transport("injected write error".into())),
        );
        assert_eq!(transport.send_feature_report(&[7, 1, 4]).unwrap(), 2);
        assert!(transport.send_feature_report(&[7, 5, 4]).is_err());
        assert!(transport.send_feature_report(&[7, 3, 4]).is_err());
        assert_eq!(
            transport.feature_reports_sent(),
            &[vec![7, 1, 4], vec![7, 5, 4], vec![7, 3, 4]]
        );
        transport.assert_drained();
    }

    #[test]
    fn mock_feature_read_error_is_consumed_once_and_counted() {
        let mut transport = MockHidTransport::new(1);
        transport.queue_feature_error(super::HidError::Transport("injected read error".into()));
        transport.queue_feature_response([7, 0x81, 4]);
        let mut report = [7, 0, 0];
        let error = transport.get_feature_report(&mut report).unwrap_err();
        assert!(error.to_string().contains("injected read error"));
        assert_eq!(transport.get_feature_report(&mut report).unwrap(), 3);
        assert_eq!(report, [7, 0x81, 4]);
        assert_eq!(transport.feature_read_attempts(), 2);
        transport.assert_drained();
    }
}

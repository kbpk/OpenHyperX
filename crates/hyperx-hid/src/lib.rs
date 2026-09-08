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
        expected_feature_tx: VecDeque<Vec<u8>>,
        feature_rx: VecDeque<Vec<u8>>,
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
            self.expected_feature_tx.push_back(report.into());
        }

        pub fn queue_feature_response(&mut self, report: impl Into<Vec<u8>>) {
            self.feature_rx.push_back(report.into());
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
            Self::check_tx(&mut self.expected_feature_tx, report)
        }

        fn get_feature_report(&mut self, report: &mut [u8]) -> Result<usize, HidError> {
            Self::receive(&mut self.feature_rx, report)
        }

        fn write(&mut self, report: &[u8]) -> Result<usize, HidError> {
            Self::check_tx(&mut self.expected_output_tx, report)
        }

        fn read_timeout(&mut self, report: &mut [u8], _timeout_ms: i32) -> Result<usize, HidError> {
            Self::receive(&mut self.input_rx, report)
        }

        fn report_descriptor(&self) -> Result<Vec<u8>, HidError> {
            Err(HidError::Transport(
                "no mock report descriptor is configured".to_owned(),
            ))
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
    }
}

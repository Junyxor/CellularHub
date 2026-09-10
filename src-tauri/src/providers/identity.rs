//! Stable local identities for cellular hardware.
//!
//! A Windows COM number is not a hardware identity: Windows may assign COM3 to a
//! different device after reboot. CellularHub therefore only persists AT-device
//! auto-receive consent when the USB transport exposes VID/PID plus a non-empty
//! serial number. The key is kept in the local preferences store and is never
//! exposed through AppSnapshot.

pub fn stable_hardware_key(device_id: &str) -> Option<String> {
    if device_id.starts_with("mbn:") {
        return Some(device_id.to_string());
    }

    #[cfg(windows)]
    {
        let port_name = device_id.strip_prefix("at:")?;
        let ports = serialport::available_ports().ok()?;
        let port = ports
            .into_iter()
            .find(|port| port.port_name.eq_ignore_ascii_case(port_name))?;
        let serialport::SerialPortType::UsbPort(info) = port.port_type else {
            return None;
        };
        return usb_hardware_key(info.vid, info.pid, info.serial_number.as_deref()?);
    }

    #[cfg(not(windows))]
    {
        let _ = device_id;
        None
    }
}

fn usb_hardware_key(vid: u16, pid: u16, serial: &str) -> Option<String> {
    let serial = serial.trim();
    if serial.is_empty() {
        return None;
    }
    Some(format!("usb:{vid:04x}:{pid:04x}:{serial}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_usb_identity_requires_serial_number() {
        assert_eq!(
            usb_hardware_key(0x2c7c, 0x0125, " ABC123 "),
            Some("usb:2c7c:0125:ABC123".into())
        );
        assert_eq!(usb_hardware_key(0x2c7c, 0x0125, "   "), None);
    }

    #[test]
    fn mbn_interface_identity_is_already_stable_enough_for_selection() {
        assert_eq!(
            stable_hardware_key("mbn:{00000000-0000-0000-0000-000000000001}"),
            Some("mbn:{00000000-0000-0000-0000-000000000001}".into())
        );
    }
}

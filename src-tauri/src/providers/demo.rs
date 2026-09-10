use crate::model::{CellularDevice, DeviceCapabilities};

pub fn demo_device() -> CellularDevice {
    CellularDevice {
        id: "demo-native".into(),
        label: "Windows Mobile Broadband".into(),
        kind: "demo".into(),
        model: Some("Demo 5G Modem".into()),
        operator: Some("CellularHub Demo".into()),
        signal: Some(82),
        network_class: Some("5G".into()),
        status: "connected".into(),
        capabilities: DeviceCapabilities {
            sms_receive: true,
            sms_send: false,
            native_esim: true,
            euicc_bridge: false,
        },
    }
}

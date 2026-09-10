pub(crate) mod at;
mod demo;
mod identity;
mod windows;

use crate::{model::CellularDevice, sms::inbox::IncomingSms};

pub use at::{AtListenerHandle as SmsListenerHandle, AtListenerStarted};
pub use demo::demo_device;
pub use identity::stable_hardware_key;
pub use windows::{detect_runtime_capabilities, launch_native_lpa};

#[derive(Debug, Clone, Default)]
pub struct ProviderPoll {
    pub messages: Vec<IncomingSms>,
    pub operator: Option<String>,
    pub signal: Option<u8>,
}

pub fn enumerate_devices() -> Vec<CellularDevice> {
    let mut devices = windows::enumerate_devices();
    let mut at_devices = at::enumerate_devices();
    devices.append(&mut at_devices);
    devices
}

pub fn read_sms(device_id: &str) -> Result<ProviderPoll, String> {
    if device_id.starts_with("at:") {
        let poll = at::poll(device_id)?;
        return Ok(ProviderPoll {
            messages: poll.messages,
            operator: poll.operator,
            signal: poll.signal,
        });
    }
    if device_id.starts_with("mbn:") {
        return Err(
            "Windows MBN SMS receive is asynchronous; the IMbnSmsEvents completion sink is not wired yet"
                .into(),
        );
    }
    Err("unknown cellular provider device id".into())
}

pub fn start_sms_listener<F>(device_id: &str, on_message: F) -> Result<AtListenerStarted, String>
where
    F: Fn(IncomingSms) + Send + 'static,
{
    if device_id.starts_with("at:") {
        return at::start_listener(device_id, on_message);
    }
    if device_id.starts_with("mbn:") {
        return Err(
            "Windows MBN listener requires the dedicated COM event worker and is not wired yet"
                .into(),
        );
    }
    Err("unknown cellular provider device id".into())
}

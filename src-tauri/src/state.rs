use std::sync::Mutex;

use crate::{
    model::{AppSnapshot, EsimProfile, SmsMessage},
    providers,
    sms::inbox::{message_from_complete, MultipartAssembler},
    store::Store,
};

pub struct AppState {
    inner: Mutex<InnerState>,
    store: Store,
}

struct InnerState {
    devices: Vec<crate::model::CellularDevice>,
    selected_device_id: Option<String>,
    messages: Vec<SmsMessage>,
    profiles: Vec<EsimProfile>,
    assembler: MultipartAssembler,
}

impl AppState {
    pub fn new(store: Store) -> Result<Self, String> {
        let mut messages = store.load_messages()?;
        messages.sort_by(|a, b| b.received_at.cmp(&a.received_at));
        let profiles = store.load_profiles()?;
        let devices = providers::enumerate_devices();
        let selected_device_id = preferred_device_id(&devices);
        Ok(Self {
            inner: Mutex::new(InnerState {
                devices,
                selected_device_id,
                messages,
                profiles,
                assembler: MultipartAssembler::default(),
            }),
            store,
        })
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let guard = self.inner.lock().expect("state poisoned");
        AppSnapshot {
            devices: guard.devices.clone(),
            selected_device_id: guard.selected_device_id.clone(),
            messages: guard.messages.clone(),
            esim_profiles: guard.profiles.clone(),
            runtime: providers::detect_runtime_capabilities(),
        }
    }

    pub fn archive_message(&self, id: &str, archived: bool) -> Result<AppSnapshot, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        if let Some(message) = guard.messages.iter_mut().find(|message| message.id == id) {
            message.archived = archived;
            if archived {
                message.unread = false;
            }
        }
        self.store.save_messages(&guard.messages)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn mark_sender_read(&self, sender: &str) -> Result<AppSnapshot, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        for message in guard.messages.iter_mut().filter(|message| message.sender == sender) {
            message.unread = false;
        }
        self.store.save_messages(&guard.messages)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn upsert_profile(&self, profile: EsimProfile) -> Result<AppSnapshot, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        if let Some(existing) = guard.profiles.iter_mut().find(|item| item.id == profile.id) {
            *existing = profile;
        } else {
            guard.profiles.insert(0, profile);
        }
        self.store.save_profiles(&guard.profiles)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn delete_profile(&self, id: &str) -> Result<AppSnapshot, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        guard.profiles.retain(|profile| profile.id != id);
        self.store.save_profiles(&guard.profiles)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn refresh_devices(&self) -> AppSnapshot {
        let refreshed = providers::enumerate_devices();
        let mut guard = self.inner.lock().expect("state poisoned");
        let previous = guard.devices.clone();
        let previous_selected = guard.selected_device_id.clone();
        guard.devices = refreshed;

        // A successful AT probe remains trusted for this process lifetime even after a passive refresh.
        for device in &mut guard.devices {
            if device.kind != "at" {
                continue;
            }
            if let Some(old) = previous.iter().find(|old| old.id == device.id && old.capabilities.sms_receive) {
                device.capabilities.sms_receive = true;
                device.status = old.status.clone();
                device.operator = old.operator.clone();
                device.signal = old.signal;
            }
        }

        guard.selected_device_id = previous_selected
            .filter(|id| guard.devices.iter().any(|device| &device.id == id))
            .or_else(|| preferred_device_id(&guard.devices));
        drop(guard);
        self.snapshot()
    }

    pub fn poll_sms_device(&self, device_id: &str) -> Result<AppSnapshot, String> {
        // Serial/COM I/O is deliberately outside the state mutex so the UI can still read snapshots.
        let poll = providers::read_sms(device_id)?;
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;

        if let Some(device) = guard.devices.iter_mut().find(|device| device.id == device_id) {
            device.status = "connected".into();
            device.capabilities.sms_receive = true;
            if poll.operator.is_some() {
                device.operator = poll.operator.clone();
            }
            if poll.signal.is_some() {
                device.signal = poll.signal;
            }
        }
        guard.selected_device_id = Some(device_id.to_string());

        let mut inserted = false;
        for incoming in poll.messages {
            if let Some(complete) = guard.assembler.push(incoming)? {
                let message = message_from_complete(complete);
                if !guard.messages.iter().any(|existing| existing.id == message.id) {
                    guard.messages.insert(0, message);
                    inserted = true;
                }
            }
        }
        guard.messages.truncate(1000);
        if inserted {
            self.store.save_messages(&guard.messages)?;
        }
        drop(guard);
        Ok(self.snapshot())
    }
}

fn preferred_device_id(devices: &[crate::model::CellularDevice]) -> Option<String> {
    devices
        .iter()
        .find(|device| device.kind == "windows-mbn" && device.capabilities.sms_receive)
        .or_else(|| devices.first())
        .map(|device| device.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CellularDevice, DeviceCapabilities};

    #[test]
    fn native_receive_capable_device_is_preferred() {
        let at = CellularDevice {
            id: "at:COM3".into(),
            label: "AT".into(),
            kind: "at".into(),
            model: None,
            operator: None,
            signal: None,
            network_class: None,
            status: "available".into(),
            capabilities: DeviceCapabilities {
                sms_receive: false,
                sms_send: false,
                native_esim: false,
                euicc_bridge: false,
            },
        };
        let mut mbn = at.clone();
        mbn.id = "mbn:test".into();
        mbn.kind = "windows-mbn".into();
        mbn.capabilities.sms_receive = true;
        assert_eq!(preferred_device_id(&[at, mbn]), Some("mbn:test".into()));
    }
}

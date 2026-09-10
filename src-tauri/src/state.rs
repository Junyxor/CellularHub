use std::sync::{Arc, Mutex};

use crate::{
    model::{AppSnapshot, EsimProfile, SmsMessage},
    providers,
    sms::inbox::{message_from_complete, IncomingSms, MultipartAssembler},
    store::Store,
};

pub struct AppState {
    inner: Mutex<InnerState>,
    listener: Mutex<Option<ActiveSmsListener>>,
    store: Store,
}

struct InnerState {
    devices: Vec<crate::model::CellularDevice>,
    selected_device_id: Option<String>,
    messages: Vec<SmsMessage>,
    profiles: Vec<EsimProfile>,
    assembler: MultipartAssembler,
}

struct ActiveSmsListener {
    device_id: String,
    _handle: providers::SmsListenerHandle,
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
            listener: Mutex::new(None),
            store,
        })
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let (devices, selected_device_id, messages, esim_profiles) = {
            let guard = self.inner.lock().expect("state poisoned");
            (
                guard.devices.clone(),
                guard.selected_device_id.clone(),
                guard.messages.clone(),
                guard.profiles.clone(),
            )
        };
        let active_sms_listener_device_id = self
            .listener
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|listener| listener.device_id.clone()));
        AppSnapshot {
            devices,
            selected_device_id,
            active_sms_listener_device_id,
            messages,
            esim_profiles,
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
        let active = self
            .listener
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|listener| listener.device_id.clone()));
        let mut guard = self.inner.lock().expect("state poisoned");
        let previous = guard.devices.clone();
        let previous_selected = guard.selected_device_id.clone();
        guard.devices = refreshed;

        // A successful AT probe remains trusted for this process lifetime even after a passive refresh.
        for device in &mut guard.devices {
            if device.kind != "at" {
                continue;
            }
            if let Some(old) = previous
                .iter()
                .find(|old| old.id == device.id && old.capabilities.sms_receive)
            {
                device.capabilities.sms_receive = true;
                device.status = old.status.clone();
                device.operator = old.operator.clone();
                device.signal = old.signal;
            }
            if active.as_deref() == Some(device.id.as_str()) {
                device.status = "connected".into();
                device.capabilities.sms_receive = true;
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
        {
            let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
            mark_device_connected(&mut guard.devices, device_id, poll.operator, poll.signal);
            guard.selected_device_id = Some(device_id.to_string());
        }
        for incoming in poll.messages {
            self.ingest_incoming(incoming)?;
        }
        Ok(self.snapshot())
    }

    pub fn start_sms_listener(
        self: &Arc<Self>,
        device_id: &str,
        on_insert: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<AppSnapshot, String> {
        if !device_id.starts_with("at:") {
            return Err("background listener is currently implemented for explicit AT devices only".into());
        }

        // Only one serial listener may own a COM port at a time. Drop the old handle outside the mutex;
        // its Drop implementation signals the worker and joins it before a new port is opened.
        let previous = {
            let mut listener = self
                .listener
                .lock()
                .map_err(|_| "listener state poisoned".to_string())?;
            if listener.as_ref().is_some_and(|active| active.device_id == device_id) {
                return Ok(self.snapshot());
            }
            listener.take()
        };
        drop(previous);

        let weak = Arc::downgrade(self);
        let notifier = on_insert.clone();
        let started = providers::start_sms_listener(device_id, move |incoming| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if matches!(state.ingest_incoming(incoming), Ok(true)) {
                notifier();
            }
        })?;

        {
            let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
            mark_device_connected(
                &mut guard.devices,
                device_id,
                started.operator.clone(),
                started.signal,
            );
            guard.selected_device_id = Some(device_id.to_string());
        }
        {
            let mut listener = self
                .listener
                .lock()
                .map_err(|_| "listener state poisoned".to_string())?;
            *listener = Some(ActiveSmsListener {
                device_id: device_id.to_string(),
                _handle: started.handle,
            });
        }
        Ok(self.snapshot())
    }

    pub fn stop_sms_listener(&self, device_id: &str) -> Result<AppSnapshot, String> {
        let stopped = {
            let mut listener = self
                .listener
                .lock()
                .map_err(|_| "listener state poisoned".to_string())?;
            match listener.as_ref() {
                Some(active) if active.device_id == device_id => listener.take(),
                _ => None,
            }
        };
        drop(stopped);
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(device) = guard.devices.iter_mut().find(|device| device.id == device_id) {
                device.status = "available".into();
            }
        }
        Ok(self.snapshot())
    }

    fn ingest_incoming(&self, incoming: IncomingSms) -> Result<bool, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        let Some(complete) = guard.assembler.push(incoming)? else {
            return Ok(false);
        };
        let message = message_from_complete(complete);
        if guard.messages.iter().any(|existing| existing.id == message.id) {
            return Ok(false);
        }
        guard.messages.insert(0, message);
        guard.messages.truncate(1000);
        self.store.save_messages(&guard.messages)?;
        Ok(true)
    }
}

fn mark_device_connected(
    devices: &mut [crate::model::CellularDevice],
    device_id: &str,
    operator: Option<String>,
    signal: Option<u8>,
) {
    if let Some(device) = devices.iter_mut().find(|device| device.id == device_id) {
        device.status = "connected".into();
        device.capabilities.sms_receive = true;
        if operator.is_some() {
            device.operator = operator;
        }
        if signal.is_some() {
            device.signal = signal;
        }
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

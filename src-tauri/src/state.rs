use std::sync::Mutex;

use crate::{model::{AppSnapshot, EsimProfile, SmsMessage}, providers, store::Store};

pub struct AppState {
    inner: Mutex<InnerState>,
    store: Store,
}

struct InnerState {
    devices: Vec<crate::model::CellularDevice>,
    selected_device_id: Option<String>,
    messages: Vec<SmsMessage>,
    profiles: Vec<EsimProfile>,
}

impl AppState {
    pub fn new(store: Store) -> Result<Self, String> {
        let messages = store.load_messages()?;
        let profiles = store.load_profiles()?;
        let devices = providers::enumerate_devices();
        let selected_device_id = devices.first().map(|d| d.id.clone());
        Ok(Self {
            inner: Mutex::new(InnerState { devices, selected_device_id, messages, profiles }),
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
        if let Some(message) = guard.messages.iter_mut().find(|m| m.id == id) {
            message.archived = archived;
            if archived { message.unread = false; }
        }
        self.store.save_messages(&guard.messages)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn mark_sender_read(&self, sender: &str) -> Result<AppSnapshot, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        for message in guard.messages.iter_mut().filter(|m| m.sender == sender) {
            message.unread = false;
        }
        self.store.save_messages(&guard.messages)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn upsert_profile(&self, profile: EsimProfile) -> Result<AppSnapshot, String> {
        let mut guard = self.inner.lock().map_err(|_| "state poisoned".to_string())?;
        if let Some(existing) = guard.profiles.iter_mut().find(|p| p.id == profile.id) {
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
        guard.profiles.retain(|p| p.id != id);
        self.store.save_profiles(&guard.profiles)?;
        drop(guard);
        Ok(self.snapshot())
    }

    pub fn refresh_devices(&self) -> AppSnapshot {
        let mut guard = self.inner.lock().expect("state poisoned");
        guard.devices = providers::enumerate_devices();
        guard.selected_device_id = guard.devices.first().map(|d| d.id.clone());
        drop(guard);
        self.snapshot()
    }
}

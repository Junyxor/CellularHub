use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::{model::{CellularSnapshot, SmsMessage}, storage::{load_json_with_backup, write_json_with_backup}};

pub struct CellularManager {
    snapshot: Mutex<CellularSnapshot>,
    messages: Mutex<Vec<SmsMessage>>,
    stop_flag: Mutex<Option<Arc<AtomicBool>>>,
    messages_path: PathBuf,
}

impl CellularManager {
    pub fn load(messages_path: PathBuf) -> Self {
        let messages = load_json_with_backup::<Vec<SmsMessage>>(&messages_path);
        Self {
            snapshot: Mutex::new(CellularSnapshot::default()),
            messages: Mutex::new(messages),
            stop_flag: Mutex::new(None),
            messages_path,
        }
    }

    fn persist_messages(&self, messages: &[SmsMessage]) {
        let _ = write_json_with_backup(&self.messages_path, messages);
    }

    pub fn snapshot(&self) -> CellularSnapshot {
        self.snapshot.lock().expect("snapshot poisoned").clone()
    }

    pub fn set_snapshot(&self, value: CellularSnapshot) {
        *self.snapshot.lock().expect("snapshot poisoned") = value;
    }

    pub fn set_error(&self, error: String) {
        let mut snap = self.snapshot.lock().expect("snapshot poisoned");
        snap.last_error = Some(error);
        snap.connected = false;
    }

    pub fn messages(&self) -> Vec<SmsMessage> {
        self.messages.lock().expect("messages poisoned").clone()
    }

    pub fn unread_count(&self) -> usize {
        self.messages
            .lock()
            .expect("messages poisoned")
            .iter()
            .filter(|message| message.unread && !message.archived)
            .count()
    }

    pub fn push_message(&self, sms: SmsMessage) -> bool {
        let mut messages = self.messages.lock().expect("messages poisoned");
        let duplicate = messages
            .iter()
            .any(|m| m.sender == sms.sender && m.body == sms.body && m.slot == sms.slot);
        if duplicate {
            return false;
        }
        messages.insert(0, sms);
        if messages.len() > 1000 {
            messages.truncate(1000);
        }
        self.persist_messages(&messages);
        true
    }

    pub fn mark_read(&self, id: &str) {
        let mut messages = self.messages.lock().expect("messages poisoned");
        if let Some(message) = messages.iter_mut().find(|m| m.id == id) {
            message.unread = false;
        }
        self.persist_messages(&messages);
    }

    pub fn mark_all_read(&self) {
        let mut messages = self.messages.lock().expect("messages poisoned");
        for message in messages.iter_mut().filter(|message| !message.archived) {
            message.unread = false;
        }
        self.persist_messages(&messages);
    }

    pub fn mark_sender_read(&self, sender: &str) {
        let mut messages = self.messages.lock().expect("messages poisoned");
        for message in messages
            .iter_mut()
            .filter(|message| message.sender == sender && !message.archived)
        {
            message.unread = false;
        }
        self.persist_messages(&messages);
    }

    pub fn set_sender_archived(&self, sender: &str, archived: bool) {
        let mut messages = self.messages.lock().expect("messages poisoned");
        for message in messages
            .iter_mut()
            .filter(|message| message.sender == sender && message.archived != archived)
        {
            message.archived = archived;
            if archived {
                message.unread = false;
            }
        }
        self.persist_messages(&messages);
    }

    pub fn set_archived(&self, id: &str, archived: bool) {
        let mut messages = self.messages.lock().expect("messages poisoned");
        if let Some(message) = messages.iter_mut().find(|m| m.id == id) {
            message.archived = archived;
            if archived {
                message.unread = false;
            }
        }
        self.persist_messages(&messages);
    }

    pub fn replace_stop_flag(&self, next: Arc<AtomicBool>) {
        if let Some(old) = self
            .stop_flag
            .lock()
            .expect("stop flag poisoned")
            .replace(next)
        {
            old.store(true, Ordering::Relaxed);
        }
    }

    pub fn disconnect(&self) {
        if let Some(flag) = self.stop_flag.lock().expect("stop flag poisoned").take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.set_snapshot(CellularSnapshot::default());
    }
}

#[cfg(test)]
mod tests {
    use super::CellularManager;
    use crate::model::{BackendKind, SmsMessage};
    use chrono::Utc;
    use uuid::Uuid;

    fn message(body: &str) -> SmsMessage {
        SmsMessage {
            id: Uuid::new_v4().to_string(),
            sender: "10010".into(),
            body: body.into(),
            received_at: Utc::now().to_rfc3339(),
            unread: true,
            archived: false,
            backend: BackendKind::Mock,
            slot: None,
        }
    }

    #[test]
    fn messages_survive_reload_and_archive() {
        let path = std::env::temp_dir().join(format!("cellularhub-test-{}.json", Uuid::new_v4()));
        let manager = CellularManager::load(path.clone());
        let sms = message("verification 482631");
        let id = sms.id.clone();
        assert!(manager.push_message(sms));
        assert_eq!(manager.unread_count(), 1);
        manager.set_archived(&id, true);
        assert_eq!(manager.unread_count(), 0);

        let second = message("another message");
        assert!(manager.push_message(second));
        manager.mark_sender_read("10010");
        assert_eq!(manager.unread_count(), 0);
        manager.set_sender_archived("10010", true);
        assert!(manager.messages().iter().all(|item| item.archived));

        let reloaded = CellularManager::load(path.clone());
        let restored = reloaded.messages();
        assert_eq!(restored.len(), 2);
        assert!(restored.iter().all(|item| item.archived));
        assert!(restored.iter().all(|item| !item.unread));
        let _ = std::fs::remove_file(path);
    }
}

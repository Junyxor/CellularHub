use std::{path::PathBuf, sync::Mutex};

use chrono::Utc;

use crate::{model::ConversationMeta, storage::{load_json_with_backup, write_json_with_backup}};

pub struct ConversationStore {
    path: PathBuf,
    items: Mutex<Vec<ConversationMeta>>,
}

impl ConversationStore {
    pub fn load(path: PathBuf) -> Self {
        let items = load_json_with_backup::<Vec<ConversationMeta>>(&path);
        Self { path, items: Mutex::new(items) }
    }

    pub fn list(&self) -> Vec<ConversationMeta> {
        self.items.lock().expect("conversation store poisoned").clone()
    }

    pub fn get(&self, sender: &str) -> Option<ConversationMeta> {
        self.items
            .lock()
            .expect("conversation store poisoned")
            .iter()
            .find(|item| item.sender == sender)
            .cloned()
    }

    pub fn save(&self, mut meta: ConversationMeta) -> Result<ConversationMeta, String> {
        meta.sender = meta.sender.trim().to_string();
        if meta.sender.is_empty() {
            return Err("sender is required".into());
        }
        meta.alias = meta.alias.trim().to_string();
        meta.notes = meta.notes.trim().to_string();
        meta.updated_at = Utc::now().to_rfc3339();
        let mut items = self.items.lock().map_err(|_| "conversation store poisoned".to_string())?;
        if let Some(existing) = items.iter_mut().find(|item| item.sender == meta.sender) {
            *existing = meta.clone();
        } else {
            items.push(meta.clone());
        }
        self.persist(&items)?;
        Ok(meta)
    }

    fn persist(&self, items: &[ConversationMeta]) -> Result<(), String> {
        write_json_with_backup(&self.path, items)
    }
}

#[cfg(test)]
mod tests {
    use super::ConversationStore;
    use crate::model::ConversationMeta;
    use uuid::Uuid;

    #[test]
    fn metadata_survives_reload() {
        let path = std::env::temp_dir().join(format!("cellularhub-meta-{}.json", Uuid::new_v4()));
        let store = ConversationStore::load(path.clone());
        store.save(ConversationMeta {
            sender: "10010".into(),
            alias: "Carrier".into(),
            notes: "data plan".into(),
            pinned: true,
            updated_at: String::new(),
        }).expect("save meta");

        let restored = ConversationStore::load(path.clone());
        let meta = restored.get("10010").expect("restored meta");
        assert_eq!(meta.alias, "Carrier");
        assert!(meta.pinned);
        let _ = std::fs::remove_file(path);
    }
}

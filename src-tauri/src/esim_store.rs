use std::{path::PathBuf, sync::Mutex};

use chrono::{Local, NaiveDate, Utc};
use uuid::Uuid;

use crate::{model::EsimProfile, storage::{load_json_with_backup, write_json_with_backup}};

pub struct EsimStore {
    path: PathBuf,
    profiles: Mutex<Vec<EsimProfile>>,
}

impl EsimStore {
    pub fn load(path: PathBuf) -> Self {
        let profiles = load_json_with_backup::<Vec<EsimProfile>>(&path);
        Self { path, profiles: Mutex::new(profiles) }
    }

    pub fn list(&self) -> Vec<EsimProfile> {
        let mut items = self.profiles.lock().expect("eSIM store poisoned").clone();
        items.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        items
    }

    pub fn save(&self, mut profile: EsimProfile) -> Result<EsimProfile, String> {
        let now = Utc::now().to_rfc3339();
        let mut items = self.profiles.lock().map_err(|_| "eSIM store poisoned".to_string())?;
        if profile.id.trim().is_empty() {
            profile.id = Uuid::new_v4().to_string();
            profile.created_at = now.clone();
        } else if profile.created_at.trim().is_empty() {
            profile.created_at = now.clone();
        }
        profile.updated_at = now;
        profile.country_code = profile.country_code.trim().to_ascii_uppercase();
        profile.tags = profile.tags.into_iter().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect();
        profile.custom_fields.retain(|f| !f.label.trim().is_empty() || !f.value.trim().is_empty());

        if let Some(existing) = items.iter_mut().find(|item| item.id == profile.id) {
            *existing = profile.clone();
        } else {
            items.push(profile.clone());
        }
        self.persist(&items)?;
        Ok(profile)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut items = self.profiles.lock().map_err(|_| "eSIM store poisoned".to_string())?;
        items.retain(|item| item.id != id);
        self.persist(&items)
    }

    pub fn due_within(&self, days: i64) -> Vec<(EsimProfile, String, i64)> {
        let today = Local::now().date_naive();
        self.list().into_iter().filter_map(|profile| {
            let mut candidates: Vec<(&str, NaiveDate)> = Vec::new();
            if let Some(value) = profile.renewal_date.as_deref().and_then(parse_date) { candidates.push(("续费", value)); }
            if let Some(value) = profile.expiry_date.as_deref().and_then(parse_date) { candidates.push(("到期", value)); }
            let (kind, date) = candidates.into_iter().min_by_key(|(_, date)| *date)?;
            let delta = (date - today).num_days();
            (delta >= 0 && delta <= days).then(|| (profile, kind.to_string(), delta))
        }).collect()
    }

    fn persist(&self, items: &[EsimProfile]) -> Result<(), String> {
        write_json_with_backup(&self.path, items).map_err(|e| format!("无法保存 eSIM 档案: {e}"))
    }
}

fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

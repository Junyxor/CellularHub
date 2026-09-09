use std::{fs, path::{Path, PathBuf}};
use serde::{de::DeserializeOwned, Serialize};
use tauri::{AppHandle, Manager};

use crate::model::{EsimProfile, SmsMessage};

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self { root })
    }

    fn read_json<T: DeserializeOwned>(&self, name: &str) -> Result<T, String>
    where
        T: Default,
    {
        let path = self.root.join(name);
        if !path.exists() {
            return Ok(T::default());
        }
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }

    fn write_json<T: Serialize + ?Sized>(&self, name: &str, value: &T) -> Result<(), String> {
        let path = self.root.join(name);
        let tmp = path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        fs::rename(tmp, path).map_err(|e| e.to_string())
    }

    pub fn load_messages(&self) -> Result<Vec<SmsMessage>, String> {
        self.read_json("messages.json")
    }

    pub fn save_messages(&self, messages: &[SmsMessage]) -> Result<(), String> {
        let capped = if messages.len() > 1000 { &messages[messages.len()-1000..] } else { messages };
        self.write_json("messages.json", capped)
    }

    pub fn load_profiles(&self) -> Result<Vec<EsimProfile>, String> {
        self.read_json("esim-profiles.json")
    }

    pub fn save_profiles(&self, profiles: &[EsimProfile]) -> Result<(), String> {
        self.write_json("esim-profiles.json", profiles)
    }

    pub fn root(&self) -> &Path { &self.root }
}

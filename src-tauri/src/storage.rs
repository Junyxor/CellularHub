use serde::{de::DeserializeOwned, Serialize};
use std::{fs, io::Write, path::{Path, PathBuf}};

fn backup_path(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("data.json");
    path.with_file_name(format!("{name}.bak"))
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("data.json");
    path.with_file_name(format!("{name}.tmp"))
}

pub fn load_json_with_backup<T: DeserializeOwned + Default>(path: &Path) -> T {
    if let Ok(bytes) = fs::read(path) {
        if let Ok(value) = serde_json::from_slice::<T>(&bytes) {
            return value;
        }
    }
    let backup = backup_path(path);
    if let Ok(bytes) = fs::read(&backup) {
        if let Ok(value) = serde_json::from_slice::<T>(&bytes) {
            return value;
        }
    }
    T::default()
}

pub fn write_json_with_backup<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let tmp = temp_path(path);
    {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    if path.exists() {
        let _ = fs::copy(path, backup_path(path));
    }
    if fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(path);
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_json_with_backup, write_json_with_backup};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
    struct Sample { value: String }

    #[test]
    fn backup_recovers_corrupt_primary() {
        let dir = std::env::temp_dir().join(format!("cellularhub-storage-{}", Uuid::new_v4()));
        let path = dir.join("sample.json");
        write_json_with_backup(&path, &Sample { value: "one".into() }).unwrap();
        write_json_with_backup(&path, &Sample { value: "two".into() }).unwrap();
        std::fs::write(&path, b"{broken").unwrap();
        let restored: Sample = load_json_with_backup(&path);
        assert_eq!(restored.value, "one");
        let _ = std::fs::remove_dir_all(dir);
    }
}

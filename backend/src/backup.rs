use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mtb::{ensure_data_dir, exec_mtb, exec_mtb_owned};
use crate::util::parse_hex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupEntry {
    pub slot: i32,
    pub path: String,
    pub bytes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Backup {
    pub id: String,
    pub time: u64,
    pub reason: String,
    pub entries: Vec<BackupEntry>,
}

pub fn create_backup(reason: &str, entries: Vec<BackupEntry>) -> Result<Backup, String> {
    let dir = ensure_data_dir().map_err(|e| format!("Data dir error: {}", e))?;
    let backups_dir = dir.join("backups");
    fs::create_dir_all(&backups_dir).map_err(|e| format!("Failed to create backups dir: {}", e))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let safe_reason = reason.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
    let id = format!("{}_{}", now, safe_reason);
    let backup = Backup {
        id: id.clone(),
        time: now,
        reason: reason.to_string(),
        entries,
    };

    let json_content = serde_json::to_string_pretty(&backup)
        .map_err(|e| format!("Failed to serialize backup: {}", e))?;

    let file_path = backups_dir.join(format!("{}.json", id));
    fs::write(&file_path, &json_content)
        .map_err(|e| format!("Failed to write backup file {:?}: {}", file_path, e))?;

    let latest_path = backups_dir.join("latest.json");
    let _ = fs::write(&latest_path, &json_content);

    Ok(backup)
}

pub fn list_backups() -> Result<Vec<Value>, String> {
    let dir = ensure_data_dir().map_err(|e| format!("Data dir error: {}", e))?;
    let backups_dir = dir.join("backups");
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&backups_dir)
        .map_err(|e| format!("Failed to read backups directory: {}", e))?;

    let mut list = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if file_name == "latest.json" {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(backup) = serde_json::from_str::<Backup>(&content) {
                        let mut val = serde_json::to_value(&backup).unwrap();
                        if let Some(obj) = val.as_object_mut() {
                            obj.insert("size".to_string(), serde_json::json!(metadata.len()));
                        }
                        list.push((backup.time, val));
                    }
                }
            }
        }
    }

    list.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(list.into_iter().map(|(_, val)| val).collect())
}

pub fn get_backup(id: &str) -> Result<Backup, String> {
    let dir = ensure_data_dir().map_err(|e| format!("Data dir error: {}", e))?;
    let backups_dir = dir.join("backups");

    let file_path = if id == "latest" {
        backups_dir.join("latest.json")
    } else if id.ends_with(".json") {
        backups_dir.join(id)
    } else {
        backups_dir.join(format!("{}.json", id))
    };

    if !file_path.exists() {
        return Err(format!("Backup '{}' not found", id));
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read backup file {:?}: {}", file_path, e))?;

    serde_json::from_str::<Backup>(&content)
        .map_err(|e| format!("Failed to parse backup JSON: {}", e))
}

pub fn restore_backup(id: &str) -> Result<Vec<Value>, String> {
    let backup = get_backup(id)?;
    let mut restored = Vec::new();

    for entry in &backup.entries {
        let (exit, ok) = if let Some(hex_bytes) = &entry.bytes {
            if let Ok(raw_bytes) = parse_hex(hex_bytes) {
                let mut write_args: Vec<String> =
                    vec!["4".into(), "5".into(), entry.slot.to_string(), entry.path.clone()];
                write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
                let (code, _) = exec_mtb_owned(write_args);
                (code, code == 0)
            } else {
                (-1, false)
            }
        } else {
            let (code, _) = exec_mtb(&[
                "4",
                "6",
                &entry.slot.to_string(),
                &entry.path,
            ]);
            (code, code == 0)
        };

        restored.push(serde_json::json!({
            "slot": entry.slot,
            "path": entry.path,
            "ok": ok,
            "exit": exit
        }));
    }

    Ok(restored)
}

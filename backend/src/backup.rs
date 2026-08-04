use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mtb::{ensure_data_dir, exec_mtb, exec_mtb_owned};
use crate::util::{
    bytes_to_hex, getprop, parse_efs_read_output, parse_hex, validate_backup_id,
    validate_nv_path, validate_slot, EfsRead,
};

pub const BACKUP_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupEntry {
    pub slot: i32,
    pub path: String,
    /// Raw bytes as hex; `None` means the item was absent (delete-restore).
    pub bytes: Option<String>,
    /// Byte count of the payload (0 when absent).
    pub size: u64,
    /// SHA-256 of the raw payload bytes, lowercase hex ("" when absent).
    pub sha256: String,
}

impl BackupEntry {
    pub fn new(slot: i32, path: String, bytes: Option<String>) -> Self {
        match bytes {
            Some(hex) => {
                let raw = parse_hex(&hex).unwrap_or_default();
                let size = raw.len() as u64;
                let sha256 = hex_sha256(&raw);
                BackupEntry { slot, path, bytes: Some(hex), size, sha256 }
            }
            None => BackupEntry {
                slot,
                path,
                bytes: None,
                size: 0,
                sha256: String::new(),
            },
        }
    }

    /// Integrity check: hex valid, size matches, checksum matches (when payload present).
    pub fn verify_integrity(&self) -> Result<(), String> {
        validate_nv_path(&self.path)?;
        match &self.bytes {
            Some(hex) => {
                if !crate::util::is_valid_sha256(&self.sha256) {
                    return Err(format!("missing/invalid checksum for {}", self.path));
                }
                let raw = parse_hex(hex)
                    .map_err(|_| format!("Backup entry has invalid hex for {}", self.path))?;
                if raw.len() as u64 != self.size {
                    return Err(format!(
                        "Backup entry size mismatch for {}: {} != {}",
                        self.path,
                        raw.len(),
                        self.size
                    ));
                }
                if hex_sha256(&raw) != self.sha256 {
                    return Err(format!("Backup entry checksum mismatch for {}", self.path));
                }
                Ok(())
            }
            None => {
                if self.size != 0 {
                    return Err(format!("Backup entry size mismatch (absent item) for {}", self.path));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Backup {
    pub version: u32,
    pub id: String,
    pub time: u64,
    pub reason: String,
    pub device: String,
    pub createdAt: String,
    pub entries: Vec<BackupEntry>,
}

fn hex_sha256(raw: &[u8]) -> String {
    let digest = Sha256::digest(raw);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn utc_now_iso() -> String {
    if let Ok(out) = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    // Fallback: epoch in ISO-ish form.
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("1970-01-01T00:00:00Z+{}s", ts)
}

pub fn create_backup(reason: &str, entries: Vec<BackupEntry>) -> Result<Backup, String> {
    let dir = ensure_data_dir().map_err(|e| format!("Data dir error: {}", e))?;
    let backups_dir = dir.join("backups");
    fs::create_dir_all(&backups_dir).map_err(|e| format!("Failed to create backups dir: {}", e))?;

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let now_secs = duration.as_secs();
    let millis = now_secs * 1000 + (duration.subsec_millis() as u64);
    let nanos = duration.subsec_nanos() as u64;
    static BACKUP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let safe_reason = reason.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
    let id = format!("{}_{}_{}_{}_{}", millis, nanos, pid, counter, safe_reason);

    let backup = Backup {
        version: BACKUP_VERSION,
        id: id.clone(),
        time: now_secs,
        reason: safe_reason,
        device: getprop("ro.product.device"),
        createdAt: utc_now_iso(),
        entries,
    };

    let json_content = serde_json::to_string_pretty(&backup)
        .map_err(|e| format!("Failed to serialize backup: {}", e))?;

    let tmp_file_path = backups_dir.join(format!("{}.json.tmp", id));
    let final_file_path = backups_dir.join(format!("{}.json", id));

    fs::write(&tmp_file_path, &json_content)
        .map_err(|e| format!("Failed to write tmp backup file {:?}: {}", tmp_file_path, e))?;

    let file = fs::File::open(&tmp_file_path)
        .map_err(|e| format!("Failed to open tmp backup file {:?}: {}", tmp_file_path, e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to fsync tmp backup file {:?}: {}", tmp_file_path, e))?;
    drop(file);

    fs::rename(&tmp_file_path, &final_file_path)
        .map_err(|e| format!("Failed to rename backup file to {:?}: {}", final_file_path, e))?;

    // Best-effort directory fsync
    if let Ok(dir_file) = fs::File::open(&backups_dir) {
        let _ = dir_file.sync_all();
    }

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
    validate_backup_id(id)?;
    let dir = ensure_data_dir().map_err(|e| format!("Data dir error: {}", e))?;
    let backups_dir = dir.join("backups");

    // "latest" resolves dynamically: the newest manifest by its `time` field.
    // No latest.json duplication — the manifest files are the single source
    // of truth (atomic tmp+fsync+rename writes make them reliable).
    let file_path = if id == "latest" {
        // Resolve the newest manifest. `time` is second-precision and several
        // backups can share a second, so ties are broken by the millis/nanos
        // embedded in the id prefix (<millis>_<nanos>_<pid>_<counter>_<reason>).
        let mut newest: Option<(u64, u64, std::path::PathBuf)> = None;
        if let Ok(rd) = fs::read_dir(&backups_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                if p.file_name().and_then(|s| s.to_str()) == Some("latest.json") {
                    continue; // legacy file, ignore
                }
                if let Ok(content) = fs::read_to_string(&p) {
                    if let Ok(b) = serde_json::from_str::<Backup>(&content) {
                        let mut parts = b.id.splitn(2, '_');
                        let millis = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                        let nanos = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                        let better = match &newest {
                            Some((m, n, _)) => millis > *m || (millis == *m && nanos > *n),
                            None => true,
                        };
                        if better {
                            newest = Some((millis, nanos, p));
                        }
                    }
                }
            }
        }
        match newest {
            Some((_, _, p)) => p,
            None => return Err("No backups found".to_string()),
        }
    } else if id.ends_with(".json") {
        backups_dir.join(id)
    } else {
        backups_dir.join(format!("{}.json", id))
    };

    if !file_path.exists() {
        return Err(format!("Backup '{}' not found", id));
    }

    let canon_file = fs::canonicalize(&file_path)
        .map_err(|e| format!("Failed to canonicalize backup path: {}", e))?;
    let canon_dir = fs::canonicalize(&backups_dir)
        .map_err(|e| format!("Failed to canonicalize backups dir: {}", e))?;

    if !canon_file.starts_with(&canon_dir) {
        return Err("invalid backup id".to_string());
    }

    let content = fs::read_to_string(&canon_file)
        .map_err(|e| format!("Failed to read backup file {:?}: {}", canon_file, e))?;

    serde_json::from_str::<Backup>(&content)
        .map_err(|e| format!("Failed to parse backup JSON: {}", e))
}

/// Restore a backup. FAILS CLOSED: every entry is integrity-verified BEFORE
/// any write happens; on a mid-restore failure all already-applied entries
/// are rolled back from the pre-restore snapshot and the rollback itself is
/// read-back verified.
pub fn restore_backup(id: &str) -> Result<Value, String> {
    let _lock = crate::mtb::FileLock::acquire().map_err(|e| e.to_string())?;

    let backup = get_backup(id)?;

    if backup.version != BACKUP_VERSION {
        return Err(format!(
            "Unsupported backup version {} (expected {})",
            backup.version, BACKUP_VERSION
        ));
    }

    // 1. Verify everything first — nothing is written until all checks pass.
    for entry in &backup.entries {
        validate_slot(entry.slot)?;
        entry.verify_integrity()?;
    }

    // 2. Pre-read every entry's current state BEFORE applying (rollback source)
    let mut snapshots: Vec<(i32, String, Option<Vec<u8>>)> = Vec::new();
    for entry in &backup.entries {
        let (exit_before, raw_before) = exec_mtb(&["4", "4", &entry.slot.to_string(), &entry.path]);
        match parse_efs_read_output(exit_before, &raw_before) {
            EfsRead::Present(b) => snapshots.push((entry.slot, entry.path.clone(), Some(b))),
            EfsRead::Absent => snapshots.push((entry.slot, entry.path.clone(), None)),
            EfsRead::Error(e) => {
                return Err(format!("Failed to pre-read {} before restore: {}", entry.path, e));
            }
        }
    }

    // 3. Apply entries in order
    let mut restored = Vec::new();
    for (idx, entry) in backup.entries.iter().enumerate() {
        let (exit, ok, verified) = match &entry.bytes {
            Some(hex_bytes) => {
                let raw_bytes = match parse_hex(hex_bytes) {
                    Ok(b) => b,
                    Err(e) => {
                        let rollback =
                            crate::util::perform_verified_rollback_entries(&snapshots[..idx]);
                        return Ok(json!({
                            "ok": false,
                            "error": format!("restore failed at {}, rolled back: {}", entry.path, e),
                            "restored": restored,
                            "rollback": rollback
                        }));
                    }
                };
                let mut write_args: Vec<String> =
                    vec!["4".into(), "5".into(), entry.slot.to_string(), entry.path.clone()];
                write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
                let (code, _) = exec_mtb_owned(write_args);
                if code != 0 {
                    (code, false, false)
                } else {
                    // read-back comparison
                    let (r_exit, r_raw) = exec_mtb(&["4", "4", &entry.slot.to_string(), &entry.path]);
                    let r_read = parse_efs_read_output(r_exit, &r_raw);
                    let verified = match r_read {
                        EfsRead::Present(b) => bytes_to_hex(&b) == *hex_bytes,
                        _ => false,
                    };
                    (code, true, verified)
                }
            }
            None => {
                let (code, _) = exec_mtb(&["4", "6", &entry.slot.to_string(), &entry.path]);
                if code != 0 {
                    (code, false, false)
                } else {
                    // item must be gone after delete
                    let (r_exit, r_raw) = exec_mtb(&["4", "4", &entry.slot.to_string(), &entry.path]);
                    let r_read = parse_efs_read_output(r_exit, &r_raw);
                    let verified = matches!(r_read, EfsRead::Absent);
                    (code, true, verified)
                }
            }
        };

        restored.push(serde_json::json!({
            "slot": entry.slot,
            "path": entry.path,
            "ok": ok,
            "exit": exit,
            "verified": verified
        }));

        if exit != 0 || !verified {
            // Rollback all applied entries (0..=idx) from snapshot, verified.
            let rollback = crate::util::perform_verified_rollback_entries(&snapshots[..=idx]);
            return Ok(json!({
                "ok": false,
                "error": format!("restore failed at {}, rolled back", entry.path),
                "restored": restored,
                "rollback": rollback
            }));
        }
    }

    Ok(json!({
        "ok": true,
        "restored": restored
    }))
}


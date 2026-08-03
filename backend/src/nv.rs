use serde_json::{json, Value};

use crate::backup::{create_backup, BackupEntry};
use crate::mtb::{exec_mtb, exec_mtb_owned, FileLock};
use crate::util::{
    bytes_to_hex, parse_efs_read_output, parse_hex, validate_hex,
    validate_nv_path, validate_slot,
};

pub fn read_nv(path: &str, slot: i32) -> Value {
    if let Err(e) = validate_nv_path(path) {
        return json!({ "ok": false, "error": e });
    }
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let (absent, bytes) = parse_efs_read_output(exit, &raw);

    json!({
        "ok": true,
        "exit": exit,
        "absent": absent,
        "bytes": bytes_to_hex(&bytes)
    })
}

pub fn write_nv(path: &str, hex_str: &str, slot: i32, reason: Option<&str>) -> Value {
    if let Err(e) = validate_nv_path(path) {
        return json!({ "ok": false, "error": e });
    }
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }
    if let Err(e) = validate_hex(hex_str) {
        return json!({ "ok": false, "error": e });
    }

    let raw_bytes = match parse_hex(hex_str) {
        Ok(b) => b,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    if raw_bytes.len() > 512 {
        return json!({ "ok": false, "error": "Write payload exceeds 512 bytes" });
    }

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    // 1. Re-read before
    let (before_exit, before_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let (before_absent, before_bytes) = parse_efs_read_output(before_exit, &before_raw);
    let before_hex = if before_absent {
        None
    } else {
        Some(bytes_to_hex(&before_bytes))
    };

    // 2. Backup before writing
    let backup_reason = reason.unwrap_or("nv_write");
    let backup_entry = BackupEntry {
        slot,
        path: path.to_string(),
        bytes: before_hex.clone(),
    };
    let backup = match create_backup(backup_reason, vec![backup_entry]) {
        Ok(b) => b,
        Err(e) => {
            return json!({
                "ok": false,
                "error": format!("Backup failed, write aborted: {}", e)
            });
        }
    };

    // 3. Write via mtb: 4 5 <slot> <path> <dec byte per arg>
    let mut write_args: Vec<String> = vec!["4".into(), "5".into(), slot.to_string(), path.to_string()];
    write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
    let (write_exit, _) = exec_mtb_owned(write_args);

    // 4. Re-read verify after
    let (after_exit, after_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let (_, after_bytes) = parse_efs_read_output(after_exit, &after_raw);

    json!({
        "ok": write_exit == 0,
        "exit": write_exit,
        "before": before_hex,
        "after": bytes_to_hex(&after_bytes),
        "backup": backup
    })
}

pub fn delete_nv(path: &str, slot: i32, reason: Option<&str>) -> Value {
    if let Err(e) = validate_nv_path(path) {
        return json!({ "ok": false, "error": e });
    }
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    // 1. Backup with null entry
    let backup_reason = reason.unwrap_or("nv_delete");
    let backup_entry = BackupEntry {
        slot,
        path: path.to_string(),
        bytes: None,
    };
    let backup = match create_backup(backup_reason, vec![backup_entry]) {
        Ok(b) => b,
        Err(e) => {
            return json!({
                "ok": false,
                "error": format!("Backup failed, delete aborted: {}", e)
            });
        }
    };

    // 2. Delete via mtb: 4 6 <slot> <path>
    let (del_exit, _) = exec_mtb(&["4", "6", &slot.to_string(), path]);

    json!({
        "ok": del_exit == 0,
        "exit": del_exit,
        "backup": backup
    })
}

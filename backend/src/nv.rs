use serde_json::{json, Value};

use crate::backup::{create_backup, BackupEntry};
use crate::mtb::{exec_mtb, exec_mtb_owned, FileLock};
use crate::util::{
    bytes_to_hex, parse_efs_read_output, parse_hex, validate_hex,
    validate_nv_path, validate_slot, EfsRead,
};

pub fn read_nv(path: &str, slot: i32) -> Value {
    if let Err(e) = validate_nv_path(path) {
        return json!({ "ok": false, "error": e });
    }
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    match parse_efs_read_output(exit, &raw) {
        EfsRead::Present(bytes) => json!({
            "ok": true,
            "exit": exit,
            "absent": false,
            "bytes": bytes_to_hex(&bytes)
        }),
        EfsRead::Absent => json!({
            "ok": true,
            "exit": exit,
            "absent": true,
            "bytes": ""
        }),
        EfsRead::Error(e) => json!({
            "ok": false,
            "exit": exit,
            "absent": Value::Null,
            "bytes": "",
            "error": e
        }),
    }
}

pub fn write_nv(path: &str, hex_str: &str, slot: i32, reason: Option<&str>) -> Value {
    // Contract: every response carries write_attempted + stage so the UI can
    // distinguish "nothing written" from "write attempted, outcome unknown".
    let err = |stage: &str, e: String| json!({
        "ok": false,
        "error": e,
        "write_attempted": false,
        "stage": stage,
        "backup_id": Value::Null,
        "verified": false,
        "rollback_attempted": false,
        "rollback_verified": false
    });

    if let Err(e) = validate_nv_path(path) {
        return err("validation", e);
    }
    if let Err(e) = validate_slot(slot) {
        return err("validation", e);
    }
    if let Err(e) = validate_hex(hex_str) {
        return err("validation", e);
    }

    let raw_bytes = match parse_hex(hex_str) {
        Ok(b) => b,
        Err(e) => return err("validation", e),
    };

    if raw_bytes.len() > 512 {
        return err("validation", "Write payload exceeds 512 bytes".to_string());
    }

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return err("lock", e),
    };

    // 1. Re-read before
    let (before_exit, before_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let before_hex = match parse_efs_read_output(before_exit, &before_raw) {
        EfsRead::Present(bytes) => Some(bytes_to_hex(&bytes)),
        EfsRead::Absent => None,
        EfsRead::Error(e) => {
            return err("read_before", format!("Read-before-write failed, aborted: {}", e));
        }
    };

    // 2. Backup before writing
    let backup_reason = reason.unwrap_or("nv_write");
    let backup_entry = BackupEntry::new(slot, path.to_string(), before_hex.clone());
    let backup = match create_backup(backup_reason, vec![backup_entry]) {
        Ok(b) => b,
        Err(e) => return err("backup", format!("Backup failed, write aborted: {}", e)),
    };
    let backup_id = backup.id.clone();

    // 3. Write via mtb: 4 5 <slot> <path> <dec byte per arg> — from here on
    // the modem write WAS attempted; outcomes may be unknown, never "nothing
    // written".
    let mut write_args: Vec<String> = vec!["4".into(), "5".into(), slot.to_string(), path.to_string()];
    write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
    let (write_exit, _) = exec_mtb_owned(write_args);

    // 4. Re-read verify after
    let (after_exit, after_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let after_read = parse_efs_read_output(after_exit, &after_raw);
    let after_bytes = match &after_read {
        EfsRead::Present(b) => b.as_slice(),
        _ => &[],
    };

    let expected = hex_str.to_lowercase();
    let is_present = matches!(after_read, EfsRead::Present(_));
    let verified = write_exit == 0 && is_present && bytes_to_hex(after_bytes).to_lowercase() == expected;
    let ok = write_exit == 0 && verified;

    let mut rollback_attempted = false;
    let mut rollback_verified = false;
    let mut rollback_obj = Value::Null;
    if write_exit == 0 && !verified {
        // verification failed — attempt auto-restore from the backup
        rollback_attempted = true;
        let before_bytes = before_hex.as_deref().and_then(|h| parse_hex(h).ok());
        let rollback_before = vec![(path.to_string(), before_bytes)];
        rollback_obj = crate::util::perform_verified_rollback(slot, &rollback_before);
        rollback_verified = rollback_obj["verified"] == true;
        json!({
            "ok": false,
            "exit": write_exit,
            "before": before_hex,
            "after": if is_present { Some(bytes_to_hex(after_bytes)) } else { None },
            "expected": expected,
            "verified": false,
            "write_attempted": true,
            "stage": if rollback_attempted { "rollback" } else { "verify" },
            "backup_id": backup_id,
            "rollback_attempted": rollback_attempted,
            "rollback_verified": rollback_verified,
            "rollback": rollback_obj,
            "backup": backup
        })
    } else {
        json!({
            "ok": ok,
            "exit": write_exit,
            "before": before_hex,
            "after": if is_present { Some(bytes_to_hex(after_bytes)) } else { None },
            "expected": expected,
            "verified": verified,
            "write_attempted": true,
            "stage": "verify",
            "backup_id": backup_id,
            "rollback_attempted": rollback_attempted,
            "rollback_verified": rollback_verified,
            "backup": backup
        })
    }
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

    // 1. Read current value FIRST (before backup)
    let (cur_exit, cur_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let before_hex = match parse_efs_read_output(cur_exit, &cur_raw) {
        EfsRead::Present(bytes) => Some(bytes_to_hex(&bytes)),
        EfsRead::Absent => None,
        EfsRead::Error(e) => {
            return json!({ "ok": false, "error": e });
        }
    };

    let backup_reason = reason.unwrap_or("nv_delete");
    let backup_entry = BackupEntry::new(slot, path.to_string(), before_hex.clone());
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

    // 3. Verify: item must be absent after delete
    let (v_exit, v_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
    let v_read = parse_efs_read_output(v_exit, &v_raw);
    let verified = del_exit == 0 && matches!(v_read, EfsRead::Absent);

    let ok = del_exit == 0 && verified;

    if del_exit == 0 && !verified {
        let before_bytes = before_hex.as_deref().and_then(|h| parse_hex(h).ok());
        let rollback_before = vec![(path.to_string(), before_bytes)];
        let rollback_obj = crate::util::perform_verified_rollback(slot, &rollback_before);
        json!({
            "ok": false,
            "exit": del_exit,
            "verified": false,
            "rollback": rollback_obj,
            "backup": backup
        })
    } else {
        json!({
            "ok": ok,
            "exit": del_exit,
            "verified": verified,
            "backup": backup
        })
    }
}

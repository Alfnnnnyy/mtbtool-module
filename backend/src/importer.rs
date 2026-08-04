use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::backup::{create_backup, BackupEntry};
use crate::mtb::{exec_mtb, exec_mtb_owned, FileLock};
use crate::util::{
    bytes_to_hex, parse_efs_read_output, parse_hex, validate_hex,
    validate_nv_path, validate_slot,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedImportCommand {
    pub slot: i32,
    pub op: String,
    pub path: String,
    pub bytes: Option<String>,
}

pub fn parse_import_json(json_str: &str) -> Result<Vec<ParsedImportCommand>, String> {
    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Malformed JSON: {}", e))?;

    let root_obj = root
        .as_object()
        .ok_or_else(|| "JSON root must be an object".to_string())?;

    if root_obj.is_empty() {
        return Err("No sim slot key found in JSON".to_string());
    }

    let valid_slot_keys = ["sim0", "sim1", "dualsim"];
    let unknown_keys: Vec<&String> = root_obj
        .keys()
        .filter(|k| !valid_slot_keys.contains(&k.as_str()))
        .collect();

    if !unknown_keys.is_empty() {
        let names: Vec<String> = unknown_keys.iter().map(|s| (*s).clone()).collect();
        return Err(format!("Unknown top-level key(s): {}", names.join(", ")));
    }

    let mut commands = Vec::new();

    for (slot_key, slot_val) in root_obj {
        let slots: Vec<i32> = match slot_key.as_str() {
            "sim0" => vec![0],
            "sim1" => vec![1],
            "dualsim" => vec![0, 1],
            _ => continue,
        };

        let slot_obj = slot_val
            .as_object()
            .ok_or_else(|| format!("Expected object for slot key: {}", slot_key))?;

        for slot in slots {
            validate_slot(slot)?;

            for (path_prefix, file_val) in slot_obj {
                let file_obj = file_val
                    .as_object()
                    .ok_or_else(|| format!("Expected object for path prefix: {}", path_prefix))?;

                for (filename, entry_val) in file_obj {
                    let entry_obj = entry_val
                        .as_object()
                        .ok_or_else(|| format!("Expected object for entry: {}", filename))?;

                    let full_path = format!("{}{}", path_prefix, filename);
                    validate_nv_path(&full_path)?;

                    let op = entry_obj
                        .get("op")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| format!("Missing or invalid 'op' for entry: {}", full_path))?;

                    match op {
                        "w" => {
                            let bytes_str = entry_obj
                                .get("data")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| format!("Missing 'data' string for write entry: {}", full_path))?;

                            validate_hex(bytes_str)?;
                            let raw_bytes = parse_hex(bytes_str)?;
                            if raw_bytes.len() > 512 {
                                return Err(format!(
                                    "Write payload for {} exceeds 512 bytes limit",
                                    full_path
                                ));
                            }

                            commands.push(ParsedImportCommand {
                                slot,
                                op: "w".to_string(),
                                path: full_path,
                                bytes: Some(bytes_str.to_lowercase()),
                            });
                        }
                        "d" => {
                            commands.push(ParsedImportCommand {
                                slot,
                                op: "d".to_string(),
                                path: full_path,
                                bytes: None,
                            });
                        }
                        _ => {
                            return Err(format!("Invalid op '{}' for entry: {}", op, full_path));
                        }
                    }
                }
            }
        }
    }

    if commands.len() > 200 {
        return Err("Import payload exceeds maximum limit of 200 entries".to_string());
    }

    Ok(commands)
}

pub fn import_preview(json_str: &str) -> Value {
    match parse_import_json(json_str) {
        Ok(cmds) => json!({
            "ok": true,
            "commands": cmds,
            "errors": []
        }),
        Err(err) => json!({
            "ok": false,
            "commands": [],
            "errors": [err]
        }),
    }
}

pub fn import_apply(json_str: &str) -> Value {
    let cmds = match parse_import_json(json_str) {
        Ok(c) => c,
        Err(err) => return json!({ "ok": false, "error": err }),
    };

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    let mut results = Vec::new();
    let mut ok_count = 0usize;
    let mut fail_count = 0usize;

    for cmd in cmds {
        // Read before
        let (exit_before, raw_before) = exec_mtb(&["4", "4", &cmd.slot.to_string(), &cmd.path]);
        let (absent_before, bytes_before) = parse_efs_read_output(exit_before, &raw_before);
        let before_hex = if absent_before { None } else { Some(bytes_to_hex(&bytes_before)) };

        // Backup
        let backup_entry = BackupEntry::new(cmd.slot, cmd.path.clone(), before_hex);
        let backup_id = match create_backup("import_apply", vec![backup_entry]) {
            Ok(b) => b.id,
            Err(_) => "".to_string(),
        };

        let (exit, ok, verified) = if cmd.op == "w" {
            if let Some(hex_bytes) = &cmd.bytes {
                if let Ok(raw) = parse_hex(hex_bytes) {
                    let mut write_args: Vec<String> =
                        vec!["4".into(), "5".into(), cmd.slot.to_string(), cmd.path.clone()];
                    write_args.extend(raw.iter().map(|b| b.to_string()));
                    let (code, _) = exec_mtb_owned(write_args);
                    if code == 0 {
                        // read-back comparison
                        let expected = bytes_to_hex(&raw);
                        let (r_exit, r_raw) = exec_mtb(&["4", "4", &cmd.slot.to_string(), &cmd.path]);
                        let (absent, r_bytes) = parse_efs_read_output(r_exit, &r_raw);
                        (code, true, !absent && bytes_to_hex(&r_bytes) == expected)
                    } else {
                        (code, false, false)
                    }
                } else {
                    (-1, false, false)
                }
            } else {
                (-1, false, false)
            }
        } else {
            let (code, _) = exec_mtb(&["4", "6", &cmd.slot.to_string(), &cmd.path]);
            if code == 0 {
                // item must be absent after delete
                let (r_exit, r_raw) = exec_mtb(&["4", "4", &cmd.slot.to_string(), &cmd.path]);
                let (absent, _) = parse_efs_read_output(r_exit, &r_raw);
                (code, true, absent)
            } else {
                (code, false, false)
            }
        };

        if ok {
            ok_count += 1;
        } else {
            fail_count += 1;
        }

        results.push(json!({
            "slot": cmd.slot,
            "op": cmd.op,
            "path": cmd.path,
            "ok": ok,
            "exit": exit,
            "verified": verified,
            "backup_id": backup_id
        }));
    }

    json!({
        "ok": fail_count == 0,
        "results": results,
        "ok_count": ok_count,
        "fail_count": fail_count
    })
}
